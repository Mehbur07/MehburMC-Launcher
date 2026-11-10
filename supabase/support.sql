-- MehburMC Launcher — admine mesaj (K101). Supabase → SQL Editor'de çalıştır,
-- sonra support_test.sql. Birden fazla çalıştırmak güvenlidir.
--
-- Oyuncu bir konu açar ve yazar; bütün adminler (rank 1 ve 2) görür ve
-- cevaplar. Banlı oyuncular da yazabilir (bana itiraz için). Tablolara
-- doğrudan erişim yok: her kural aşağıdaki fonksiyonlarda, launcher'a
-- güvenilmez.

create table if not exists public.support_threads (
  id bigint generated always as identity primary key,
  owner uuid not null references auth.users (id) on delete cascade,
  subject text not null check (char_length(subject) between 1 and 80),
  status text not null default 'open' check (status in ('open', 'closed')),
  -- Son mesajın id'si; okunmamış sayısı ve sıralama için.
  last_message bigint not null default 0,
  last_from_admin boolean not null default false,
  -- Her iki tarafın en son okuduğu mesaj (adminler tek taraf sayılır).
  owner_seen bigint not null default 0,
  admin_seen bigint not null default 0,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);
create index if not exists support_threads_owner on public.support_threads (owner, updated_at desc);
create index if not exists support_threads_status on public.support_threads (status, updated_at desc);

create table if not exists public.support_messages (
  id bigint generated always as identity primary key,
  thread bigint not null references public.support_threads (id) on delete cascade,
  sender uuid references auth.users (id) on delete set null,
  from_admin boolean not null,
  body text not null check (char_length(body) between 1 and 2000),
  created_at timestamptz not null default now()
);
create index if not exists support_messages_thread on public.support_messages (thread, id);
create index if not exists support_messages_sender on public.support_messages (sender, created_at);

alter table public.support_threads enable row level security;
alter table public.support_messages enable row level security;
revoke all on public.support_threads, public.support_messages from anon, authenticated;

-- ---------------------------------------------------------------- yardımcılar

-- Banlı olmayan admin mi (rank 1 ya da 2)?
create or replace function public.support_is_admin(p_user uuid)
returns boolean language sql stable security definer set search_path = public as $$
  select public.is_member() and public.admin_level(p_user) >= 1 and not public.is_banned(p_user);
$$;

-- Mesajı ekler, konuyu günceller; yazanın kendi mesajı okunmamış sayılmaz.
create or replace function public.support_post(
  p_thread bigint, p_sender uuid, p_from_admin boolean, p_body text
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  v_body text := btrim(coalesce(p_body, ''));
  v_id bigint;
begin
  if char_length(v_body) not between 1 and 2000 then
    raise exception 'friends.messageInvalid' using errcode = 'P0001';
  end if;
  insert into support_messages (thread, sender, from_admin, body)
  values (p_thread, p_sender, p_from_admin, v_body)
  returning id into v_id;
  update support_threads t
     set last_message = v_id,
         last_from_admin = p_from_admin,
         updated_at = now(),
         owner_seen = case when p_from_admin then t.owner_seen else v_id end,
         admin_seen = case when p_from_admin then v_id else t.admin_seen end
   where t.id = p_thread;
  return v_id;
end;
$$;

revoke execute on function
  public.support_is_admin(uuid), public.support_post(bigint, uuid, boolean, text)
from public, anon, authenticated;

-- ---------------------------------------------------------------- oyuncu

-- Yeni konu açar ve ilk mesajı yazar; konunun id'sini döndürür.
create or replace function public.support_open(p_subject text, p_body text)
returns bigint language plpgsql security definer set search_path = public as $$
declare
  v_me uuid := auth.uid();
  v_subject text := left(btrim(regexp_replace(coalesce(p_subject, ''), '\s+', ' ', 'g')), 80);
  v_id bigint;
begin
  if not public.is_member() then
    raise exception 'support.membersOnly' using errcode = 'P0001';
  end if;
  if v_subject = '' then
    raise exception 'support.subjectInvalid' using errcode = 'P0001';
  end if;
  if (select count(*) from support_threads t where t.owner = v_me and t.status = 'open') >= 5 then
    raise exception 'support.tooManyOpen' using errcode = 'P0001';
  end if;
  if (select count(*) from support_threads t
       where t.owner = v_me and t.created_at > now() - interval '1 day') >= 3 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into support_threads (owner, subject) values (v_me, v_subject) returning id into v_id;
  perform public.support_post(v_id, v_me, false, p_body);
  return v_id;
end;
$$;

-- Konuya yazar: sahibi açık konusuna, adminler her konuya (kapalı konuyu
-- yeniden açar).
create or replace function public.support_send(p_thread bigint, p_body text)
returns bigint language plpgsql security definer set search_path = public as $$
declare
  v_me uuid := auth.uid();
  v_thread support_threads;
begin
  select * into v_thread from support_threads t where t.id = p_thread;
  if v_thread.id is not null and v_thread.owner = v_me and public.is_member() then
    if v_thread.status <> 'open' then
      raise exception 'support.closed' using errcode = 'P0001';
    end if;
    if (select count(*) from support_messages m
         where m.sender = v_me and not m.from_admin
           and m.created_at > now() - interval '1 hour') >= 30 then
      raise exception 'friends.rateLimited' using errcode = 'P0001';
    end if;
    return public.support_post(p_thread, v_me, false, p_body);
  end if;
  if v_thread.id is not null and public.support_is_admin(v_me) then
    update support_threads t set status = 'open' where t.id = p_thread;
    return public.support_post(p_thread, v_me, true, p_body);
  end if;
  raise exception 'support.notFound' using errcode = 'P0001';
end;
$$;

-- Oyuncunun kendi konuları; açık olanlar ve son yazılanlar önce.
create or replace function public.support_my_threads()
returns table (
  id bigint, subject text, status text, created_at timestamptz, updated_at timestamptz,
  unread int
) language sql stable security definer set search_path = public as $$
  select t.id, t.subject, t.status, t.created_at, t.updated_at,
         (select count(*)::int from support_messages m
           where m.thread = t.id and m.from_admin and m.id > t.owner_seen)
    from support_threads t
   where t.owner = auth.uid()
   order by t.status = 'open' desc, t.updated_at desc
   limit 100;
$$;

-- Bir konunun mesajları (sahibi ya da admin). Oyuncu adminin adını görmez,
-- yalnızca "yönetici" yazar; adminler kimin yazdığını görür.
create or replace function public.support_thread_messages(p_thread bigint, p_after bigint default 0)
returns table (
  id bigint, from_admin boolean, author text, body text, created_at timestamptz, mine boolean
) language plpgsql stable security definer set search_path = public as $$
declare
  v_me uuid := auth.uid();
  v_admin boolean := public.support_is_admin(v_me);
  v_owner uuid;
begin
  select t.owner into v_owner from support_threads t where t.id = p_thread;
  if v_owner is null or (v_owner <> v_me and not v_admin) then
    raise exception 'support.notFound' using errcode = 'P0001';
  end if;
  return query
    select m.id, m.from_admin,
           case when v_admin then (public.user_names(m.sender))[1] end,
           m.body, m.created_at, m.sender = v_me
      from support_messages m
     where m.thread = p_thread and m.id > coalesce(p_after, 0)
     order by m.id
     limit 500;
end;
$$;

-- Konuyu okundu sayar (sahibi kendi tarafını, admin admin tarafını).
create or replace function public.support_mark_read(p_thread bigint)
returns void language plpgsql security definer set search_path = public as $$
declare v_me uuid := auth.uid();
begin
  update support_threads t set owner_seen = t.last_message
   where t.id = p_thread and t.owner = v_me;
  if not found and public.support_is_admin(v_me) then
    update support_threads t set admin_seen = t.last_message where t.id = p_thread;
  end if;
end;
$$;

-- Kapatır ya da yeniden açar: sahibi yalnızca kapatabilir, adminler ikisini de.
create or replace function public.support_set_open(p_thread bigint, p_open boolean)
returns void language plpgsql security definer set search_path = public as $$
declare
  v_me uuid := auth.uid();
  v_owner uuid;
begin
  select t.owner into v_owner from support_threads t where t.id = p_thread;
  if v_owner is null then
    raise exception 'support.notFound' using errcode = 'P0001';
  end if;
  if public.support_is_admin(v_me) and v_owner <> v_me then
    update support_threads t set status = case when p_open then 'open' else 'closed' end
     where t.id = p_thread;
    perform public.admin_note(v_me, case when p_open then 'reopenSupport' else 'closeSupport' end,
                              p_thread::text, '');
  elsif v_owner = v_me and not p_open then
    update support_threads t set status = 'closed' where t.id = p_thread;
  else
    raise exception 'support.notFound' using errcode = 'P0001';
  end if;
end;
$$;

-- Rozetler için: oyuncunun okunmamış cevapları ve (admin ise) cevap bekleyen
-- konular.
create or replace function public.support_unread()
returns table (mine int, admin int) language sql stable security definer set search_path = public as $$
  select
    (select count(*)::int from support_messages m
       join support_threads t on t.id = m.thread
      where t.owner = auth.uid() and m.from_admin and m.id > t.owner_seen),
    case when public.support_is_admin(auth.uid()) then
      (select count(*)::int from support_threads t
        where t.status = 'open' and t.owner <> auth.uid() and t.last_message > t.admin_seen)
    else 0 end;
$$;

-- ---------------------------------------------------------------- admin

-- Açık ya da kapalı konular; cevap bekleyenler önce.
create or replace function public.admin_support_threads(p_open boolean)
returns table (
  id bigint, owner uuid, names text[], banned boolean, subject text, status text,
  created_at timestamptz, updated_at timestamptz, unread int, last_from_admin boolean,
  preview text
) language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(1);
  return query
    select t.id, t.owner, public.user_names(t.owner), public.is_banned(t.owner), t.subject,
           t.status, t.created_at, t.updated_at,
           (select count(*)::int from support_messages m
             where m.thread = t.id and not m.from_admin and m.id > t.admin_seen),
           t.last_from_admin,
           (select left(m.body, 140) from support_messages m where m.id = t.last_message)
      from support_threads t
     where t.status = case when p_open then 'open' else 'closed' end
     order by (t.last_message > t.admin_seen) desc, t.updated_at desc
     limit 200;
end;
$$;

revoke execute on function
  public.support_open(text, text), public.support_send(bigint, text),
  public.support_my_threads(), public.support_thread_messages(bigint, bigint),
  public.support_mark_read(bigint), public.support_set_open(bigint, boolean),
  public.support_unread(), public.admin_support_threads(boolean)
from public, anon;
grant execute on function
  public.support_open(text, text), public.support_send(bigint, text),
  public.support_my_threads(), public.support_thread_messages(bigint, bigint),
  public.support_mark_read(bigint), public.support_set_open(bigint, boolean),
  public.support_unread(), public.admin_support_threads(boolean)
to authenticated;
