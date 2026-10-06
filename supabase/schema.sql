-- MehburMC Launcher — Friends backend (ARCHITECTURE K64).
-- Run once in Supabase → SQL Editor. Safe to re-run: every object is created
-- with "if not exists" / "or replace", policies are dropped and recreated.
-- Requires: Authentication → Allow anonymous sign-ins = ON.
-- Later phases are appended below and also shipped as supabase/phaseNN.sql
-- for projects that already ran an earlier version.

create extension if not exists pgcrypto;

-- ---------------------------------------------------------------- tables

create table if not exists public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  friend_code text not null unique,
  display_name text not null check (char_length(display_name) between 1 and 32),
  created_at timestamptz not null default now()
);

create table if not exists public.friendships (
  id bigint generated always as identity primary key,
  requester uuid not null references public.profiles (id) on delete cascade,
  addressee uuid not null references public.profiles (id) on delete cascade,
  status text not null check (status in ('pending', 'accepted', 'blocked')),
  -- Who blocked (only for status = 'blocked').
  blocked_by uuid references public.profiles (id) on delete cascade,
  created_at timestamptz not null default now(),
  check (requester <> addressee)
);
-- One row per pair, whichever side asked.
create unique index if not exists friendships_pair
  on public.friendships (least(requester, addressee), greatest(requester, addressee));

create table if not exists public.messages (
  id bigint generated always as identity primary key,
  sender uuid not null references public.profiles (id) on delete cascade,
  recipient uuid not null references public.profiles (id) on delete cascade,
  body text not null check (char_length(body) between 1 and 2000),
  created_at timestamptz not null default now(),
  read_at timestamptz
);
create index if not exists messages_pair on public.messages (sender, recipient, id);
create index if not exists messages_recipient on public.messages (recipient, id);

create table if not exists public.shared_lists (
  id bigint generated always as identity primary key,
  owner uuid not null references public.profiles (id) on delete cascade,
  -- Launcher-side instance id; one list per instance and owner.
  instance_id text not null check (char_length(instance_id) between 1 and 64),
  instance_name text not null check (char_length(instance_name) between 1 and 64),
  mc_version text not null check (char_length(mc_version) <= 32),
  loader text not null check (char_length(loader) <= 32),
  items jsonb not null check (jsonb_typeof(items) = 'array' and jsonb_array_length(items) <= 1000),
  updated_at timestamptz not null default now(),
  unique (owner, instance_id)
);

create or replace function public.touch_updated_at()
returns trigger language plpgsql as $$
begin
  new.updated_at := now();
  return new;
end;
$$;
drop trigger if exists shared_lists_touch on public.shared_lists;
create trigger shared_lists_touch before insert or update on public.shared_lists
  for each row execute function public.touch_updated_at();

-- ---------------------------------------------------------------- helpers

create or replace function public.are_friends(a uuid, b uuid)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from friendships
    where status = 'accepted'
      and least(requester, addressee) = least(a, b)
      and greatest(requester, addressee) = greatest(a, b)
  );
$$;

-- 'MEHBUR-' + 4 characters without look-alikes (no 0/O, 1/I/L).
create or replace function public.new_friend_code()
returns text language plpgsql volatile set search_path = public as $$
declare
  alphabet constant text := '23456789ABCDEFGHJKMNPQRSTUVWXYZ';
  code text;
begin
  loop
    code := 'MEHBUR-';
    for i in 1..4 loop
      code := code || substr(alphabet, 1 + floor(random() * length(alphabet))::int, 1);
    end loop;
    exit when not exists (select 1 from profiles where friend_code = code);
  end loop;
  return code;
end;
$$;

-- ---------------------------------------------------------------- RPCs

-- Creates (or renames) the caller's profile; returns it.
create or replace function public.ensure_profile(name text)
returns profiles language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean text := left(btrim(coalesce(name, '')), 32);
  p profiles;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if clean = '' then clean := 'Player'; end if;
  insert into profiles (id, friend_code, display_name)
  values (me, new_friend_code(), clean)
  on conflict (id) do update set display_name = excluded.display_name
  returning * into p;
  return p;
end;
$$;

-- Friend request by code. Accepts a pending request from the other side.
create or replace function public.send_request(code text)
returns text language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  other uuid;
  f friendships;
begin
  select id into other from profiles where friend_code = upper(btrim(code));
  if other is null then raise exception 'friends.codeNotFound' using errcode = 'P0001'; end if;
  if other = me then raise exception 'friends.self' using errcode = 'P0001'; end if;
  select * into f from friendships
   where least(requester, addressee) = least(me, other)
     and greatest(requester, addressee) = greatest(me, other);
  if f.id is null then
    insert into friendships (requester, addressee, status) values (me, other, 'pending');
    return 'pending';
  elsif f.status = 'accepted' then
    raise exception 'friends.alreadyFriends' using errcode = 'P0001';
  elsif f.status = 'blocked' then
    -- Do not reveal the block to the blocked side.
    return 'pending';
  elsif f.requester = other then
    update friendships set status = 'accepted' where id = f.id;
    return 'accepted';
  end if;
  return 'pending';
end;
$$;

create or replace function public.respond_request(request_id bigint, accept boolean)
returns void language plpgsql security definer set search_path = public as $$
begin
  if accept then
    update friendships set status = 'accepted'
     where id = request_id and addressee = auth.uid() and status = 'pending';
  else
    delete from friendships
     where id = request_id and addressee = auth.uid() and status = 'pending';
  end if;
end;
$$;

-- Removes a friend, cancels/declines a request or lifts the caller's block.
create or replace function public.remove_friend(friend uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  delete from friendships
   where least(requester, addressee) = least(me, friend)
     and greatest(requester, addressee) = greatest(me, friend)
     and (status <> 'blocked' or blocked_by = me);
end;
$$;

create or replace function public.block_user(target uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if target = me then return; end if;
  insert into friendships (requester, addressee, status, blocked_by)
  values (me, target, 'blocked', me)
  on conflict (least(requester, addressee), greatest(requester, addressee))
  do update set status = 'blocked', blocked_by = me;
end;
$$;

create or replace function public.mark_read(friend uuid)
returns void language sql security definer set search_path = public as $$
  update messages set read_at = now()
   where recipient = auth.uid() and sender = friend and read_at is null;
$$;

-- Friends, requests and blocks of the caller, with unread counts.
create or replace function public.my_friends()
returns table (
  id uuid, friend_code text, display_name text, status text,
  incoming boolean, request_id bigint, unread bigint, blocked_by_me boolean
) language sql stable security definer set search_path = public as $$
  select p.id, p.friend_code, p.display_name, f.status,
         f.addressee = auth.uid() as incoming, f.id,
         (select count(*) from messages m
           where m.sender = p.id and m.recipient = auth.uid() and m.read_at is null),
         coalesce(f.blocked_by = auth.uid(), false)
    from friendships f
    join profiles p on p.id = case when f.requester = auth.uid() then f.addressee else f.requester end
   where (f.requester = auth.uid() or f.addressee = auth.uid())
     -- The blocked side does not see the relationship at all.
     and (f.status <> 'blocked' or f.blocked_by = auth.uid())
   order by p.display_name;
$$;

-- Deletes everything of the caller (profile, friendships, messages, lists)
-- and the anonymous auth user itself. Uploaded files are removed by the
-- launcher through the Storage API first: Supabase forbids deleting
-- storage.objects rows from SQL.
create or replace function public.delete_me()
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if me is null then return; end if;
  delete from profiles where id = me;
  delete from auth.users where id = me;
end;
$$;

-- ---------------------------------------------------------------- rate limit

create or replace function public.messages_rate_limit()
returns trigger language plpgsql security definer set search_path = public as $$
begin
  if (select count(*) from messages
       where sender = new.sender and created_at > now() - interval '1 minute') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  return new;
end;
$$;
drop trigger if exists messages_rate_limit on public.messages;
create trigger messages_rate_limit before insert on public.messages
  for each row execute function public.messages_rate_limit();

-- ---------------------------------------------------------------- RLS

alter table public.profiles enable row level security;
alter table public.friendships enable row level security;
alter table public.messages enable row level security;
alter table public.shared_lists enable row level security;

drop policy if exists profiles_read on public.profiles;
create policy profiles_read on public.profiles for select to authenticated
  using (id = auth.uid() or are_friends(auth.uid(), id));

-- Friendships change only through the RPCs above.
drop policy if exists friendships_read on public.friendships;
create policy friendships_read on public.friendships for select to authenticated
  using ((requester = auth.uid() or addressee = auth.uid())
         and (status <> 'blocked' or blocked_by = auth.uid()));

drop policy if exists messages_read on public.messages;
create policy messages_read on public.messages for select to authenticated
  using (sender = auth.uid() or recipient = auth.uid());
drop policy if exists messages_send on public.messages;
create policy messages_send on public.messages for insert to authenticated
  with check (sender = auth.uid() and read_at is null and are_friends(sender, recipient));

drop policy if exists lists_read on public.shared_lists;
create policy lists_read on public.shared_lists for select to authenticated
  using (owner = auth.uid() or are_friends(auth.uid(), owner));
drop policy if exists lists_write on public.shared_lists;
create policy lists_write on public.shared_lists for all to authenticated
  using (owner = auth.uid()) with check (owner = auth.uid());

revoke all on function public.new_friend_code() from public, anon, authenticated;
revoke all on function public.messages_rate_limit() from public, anon, authenticated;
grant execute on function public.ensure_profile(text), public.send_request(text),
  public.respond_request(bigint, boolean), public.remove_friend(uuid), public.block_user(uuid),
  public.mark_read(uuid), public.my_friends(), public.delete_me()
  to authenticated;

-- ---------------------------------------------------------------- storage

insert into storage.buckets (id, name, public, file_size_limit)
values ('mods', 'mods', false, 52428800)
on conflict (id) do update set public = false, file_size_limit = 52428800;

drop policy if exists mods_upload on storage.objects;
create policy mods_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'mods' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists mods_update on storage.objects;
create policy mods_update on storage.objects for update to authenticated
  using (bucket_id = 'mods' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists mods_delete on storage.objects;
create policy mods_delete on storage.objects for delete to authenticated
  using (bucket_id = 'mods' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists mods_read on storage.objects;
create policy mods_read on storage.objects for select to authenticated
  using (bucket_id = 'mods' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.are_friends(auth.uid(), ((storage.foldername(name))[1])::uuid)));

-- ================================================================ phase 12

alter table public.profiles add column if not exists avatar_sha1 text
  check (avatar_sha1 ~ '^[0-9a-f]{40}$');

-- The caller's photo version (null = no photo). The file itself lives in
-- the `avatars` bucket as <uid>/<sha1>.png and is uploaded first.
create or replace function public.set_avatar(sha1 text)
returns void language sql security definer set search_path = public as $$
  update profiles set avatar_sha1 = lower(sha1) where id = auth.uid();
$$;

-- my_friends() gains avatar_sha1 (accepted friends only; their photo is
-- readable only to friends). Return type changes, so drop first.
drop function if exists public.my_friends();
create function public.my_friends()
returns table (
  id uuid, friend_code text, display_name text, status text,
  incoming boolean, request_id bigint, unread bigint, blocked_by_me boolean,
  avatar_sha1 text
) language sql stable security definer set search_path = public as $$
  select p.id, p.friend_code, p.display_name, f.status,
         f.addressee = auth.uid() as incoming, f.id,
         (select count(*) from messages m
           where m.sender = p.id and m.recipient = auth.uid() and m.read_at is null),
         coalesce(f.blocked_by = auth.uid(), false),
         case when f.status = 'accepted' then p.avatar_sha1 end
    from friendships f
    join profiles p on p.id = case when f.requester = auth.uid() then f.addressee else f.requester end
   where (f.requester = auth.uid() or f.addressee = auth.uid())
     -- The blocked side does not see the relationship at all.
     and (f.status <> 'blocked' or f.blocked_by = auth.uid())
   order by p.display_name;
$$;

grant execute on function public.set_avatar(text), public.my_friends() to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('avatars', 'avatars', false, 262144, array['image/png'])
on conflict (id) do update
  set public = false, file_size_limit = 262144, allowed_mime_types = array['image/png'];

drop policy if exists avatars_upload on storage.objects;
create policy avatars_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'avatars' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists avatars_update on storage.objects;
create policy avatars_update on storage.objects for update to authenticated
  using (bucket_id = 'avatars' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists avatars_delete on storage.objects;
create policy avatars_delete on storage.objects for delete to authenticated
  using (bucket_id = 'avatars' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists avatars_read on storage.objects;
create policy avatars_read on storage.objects for select to authenticated
  using (bucket_id = 'avatars' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.are_friends(auth.uid(), ((storage.foldername(name))[1])::uuid)));

-- ================================================================ phase 13

alter table public.profiles add column if not exists last_seen timestamptz;

-- Sent by an open launcher every 60 s while friends are on.
create or replace function public.heartbeat()
returns void language sql security definer set search_path = public as $$
  update profiles set last_seen = now() where id = auth.uid();
$$;

-- Sent when the launcher closes (best effort; otherwise it times out).
create or replace function public.go_offline()
returns void language sql security definer set search_path = public as $$
  update profiles set last_seen = null where id = auth.uid();
$$;

-- my_friends() gains `online` (accepted friends seen in the last 90 s).
-- Return type changes, so drop first.
drop function if exists public.my_friends();
create function public.my_friends()
returns table (
  id uuid, friend_code text, display_name text, status text,
  incoming boolean, request_id bigint, unread bigint, blocked_by_me boolean,
  avatar_sha1 text, online boolean
) language sql stable security definer set search_path = public as $$
  select p.id, p.friend_code, p.display_name, f.status,
         f.addressee = auth.uid() as incoming, f.id,
         (select count(*) from messages m
           where m.sender = p.id and m.recipient = auth.uid() and m.read_at is null),
         coalesce(f.blocked_by = auth.uid(), false),
         case when f.status = 'accepted' then p.avatar_sha1 end,
         f.status = 'accepted' and coalesce(p.last_seen > now() - interval '90 seconds', false)
    from friendships f
    join profiles p on p.id = case when f.requester = auth.uid() then f.addressee else f.requester end
   where (f.requester = auth.uid() or f.addressee = auth.uid())
     -- The blocked side does not see the relationship at all.
     and (f.status <> 'blocked' or f.blocked_by = auth.uid())
   order by p.display_name;
$$;

grant execute on function public.heartbeat(), public.go_offline(), public.my_friends()
  to authenticated;

-- ================================================================ phase 14

-- One row per reserved name. Owned by the installation's anonymous
-- identity, not by the friends profile: turning friends off keeps names.
create table if not exists public.account_names (
  name_lower text primary key check (name_lower ~ '^[a-z0-9_]{3,16}$'),
  name text not null check (name ~ '^[A-Za-z0-9_]{3,16}$' and lower(name) = name_lower),
  owner uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now()
);
create index if not exists account_names_owner on public.account_names (owner);

-- No policies: the table is reachable only through the functions below, so
-- nobody can list other people's names.
alter table public.account_names enable row level security;
revoke all on public.account_names from anon, authenticated;

-- Reserves `name` for the caller. Same owner = no-op (spelling updated).
create or replace function public.claim_name(name text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean text := btrim(coalesce(name, ''));
  holder uuid;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if clean !~ '^[A-Za-z0-9_]{3,16}$' then
    raise exception 'account.nameInvalid' using errcode = 'P0001';
  end if;
  select owner into holder from account_names where name_lower = lower(clean);
  if holder = me then
    update account_names set name = clean where name_lower = lower(clean);
    return;
  elsif holder is not null then
    raise exception 'account.nameTaken' using errcode = 'P0001';
  end if;
  if (select count(*) from account_names where owner = me) >= 10 then
    raise exception 'account.nameLimit' using errcode = 'P0001';
  end if;
  insert into account_names (name_lower, name, owner) values (lower(clean), clean, me)
  on conflict (name_lower) do nothing;
  if not found then
    raise exception 'account.nameTaken' using errcode = 'P0001';
  end if;
end;
$$;

create or replace function public.release_name(name text)
returns void language sql security definer set search_path = public as $$
  delete from account_names where name_lower = lower(btrim(name)) and owner = auth.uid();
$$;

-- Atomic rename: one transaction, so if the new name is taken the old one
-- is kept (the delete is rolled back with the error).
create or replace function public.rename_name(old_name text, new_name text)
returns void language plpgsql security definer set search_path = public as $$
begin
  if lower(btrim(old_name)) <> lower(btrim(new_name)) then
    -- Frees a slot first so a rename never trips the per-identity limit.
    delete from account_names where name_lower = lower(btrim(old_name)) and owner = auth.uid();
  end if;
  perform claim_name(new_name);
end;
$$;

-- Startup sync of existing local accounts: claims what it can and reports
-- each name as 'ok', 'taken' or 'limit'.
create or replace function public.claim_names(names text[])
returns table (name text, result text)
language plpgsql security definer set search_path = public as $$
declare n text;
begin
  foreach n in array coalesce(names, '{}') loop
    begin
      perform claim_name(n);
      name := n; result := 'ok';
    exception when others then
      name := n;
      result := case sqlerrm when 'account.nameLimit' then 'limit'
                             when 'account.nameInvalid' then 'invalid'
                             else 'taken' end;
    end;
    return next;
  end loop;
end;
$$;

-- "Turn off friends and delete my data": profile, friendships, messages and
-- lists go (cascade); the identity and its reserved names stay.
create or replace function public.delete_friend_data()
returns void language sql security definer set search_path = public as $$
  delete from profiles where id = auth.uid();
$$;

grant execute on function public.claim_name(text), public.release_name(text),
  public.rename_name(text, text), public.claim_names(text[]), public.delete_friend_data()
  to authenticated;

-- ---------------------------------------------------------------- phase 17: shared skins/capes (K70)


-- One row per shared texture. The PNG lives in the private `textures`
-- bucket as <owner>/<sha1>.png and is uploaded before the row is created.
create table if not exists public.shared_textures (
  id bigint generated always as identity primary key,
  owner uuid not null references auth.users (id) on delete cascade,
  kind text not null check (kind in ('skin', 'cape')),
  model text not null default 'classic' check (model in ('classic', 'slim')),
  name text not null check (char_length(btrim(name)) between 1 and 48),
  -- One of the owner's reserved account names (account_names, K67).
  author text not null check (author ~ '^[A-Za-z0-9_]{3,16}$'),
  sha1 text not null check (sha1 ~ '^[0-9a-f]{40}$'),
  visibility text not null check (visibility in ('public', 'friends')),
  -- Set by admins (phase 20); hidden rows are visible to nobody but the owner.
  hidden boolean not null default false,
  created_at timestamptz not null default now(),
  unique (owner, sha1)
);
create index if not exists shared_textures_recent on public.shared_textures (created_at desc);

-- "Report" from other users; the admin view comes with phase 20.
create table if not exists public.texture_reports (
  id bigint generated always as identity primary key,
  texture_id bigint not null references public.shared_textures (id) on delete cascade,
  reporter uuid not null references auth.users (id) on delete cascade,
  reason text not null check (reason in ('inappropriate', 'stolen', 'spam', 'other')),
  note text check (char_length(note) <= 500),
  created_at timestamptz not null default now(),
  unique (texture_id, reporter)
);

-- Reachable only through the functions below.
alter table public.shared_textures enable row level security;
alter table public.texture_reports enable row level security;
revoke all on public.shared_textures, public.texture_reports from anon, authenticated;

-- Shares (or updates) the caller's texture. Returns the row id.
create or replace function public.share_texture(
  kind text, model text, name text, author text, sha1 text, visibility text
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(sha1, ''));
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  if not exists (select 1 from storage.objects o
                  where o.bucket_id = 'textures' and o.name = me::text || '/' || clean_sha || '.png') then
    raise exception 'textures.fileMissing' using errcode = 'P0001';
  end if;
  select t.id into existing from shared_textures t where t.owner = me and t.sha1 = clean_sha;
  if existing is null then
    if (select count(*) from shared_textures t where t.owner = me) >= 30 then
      raise exception 'textures.quota' using errcode = 'P0001';
    end if;
    if (select count(*) from shared_textures t
         where t.owner = me and t.created_at > now() - interval '1 hour') >= 10 then
      raise exception 'friends.rateLimited' using errcode = 'P0001';
    end if;
  end if;
  insert into shared_textures as t (owner, kind, model, name, author, sha1, visibility)
  values (me, kind, coalesce(model, 'classic'), btrim(name), btrim(author), clean_sha, visibility)
  on conflict on constraint shared_textures_owner_sha1_key do update
    set kind = excluded.kind, model = excluded.model, name = excluded.name,
        author = excluded.author, visibility = excluded.visibility
  returning t.id into out_id;
  return out_id;
end;
$$;

-- Withdraws the caller's share; returns its SHA-1 so the launcher can
-- delete the file (SQL may not delete storage objects).
create or replace function public.unshare_texture(texture_id bigint)
returns text language plpgsql security definer set search_path = public as $$
declare out_sha text;
begin
  delete from shared_textures t where t.id = texture_id and t.owner = auth.uid()
  returning t.sha1 into out_sha;
  if out_sha is null then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  return out_sha;
end;
$$;

-- Whether the caller may see a share.
create or replace function public.can_see_texture(t public.shared_textures)
returns boolean language sql stable security definer set search_path = public as $$
  select t.owner = auth.uid()
      or (not t.hidden and (t.visibility = 'public'
          or (t.visibility = 'friends' and are_friends(auth.uid(), t.owner))));
$$;

-- Shares the caller can see (own first), minus the ones they reported.
create or replace function public.community_textures(max_rows int default 300)
returns table (
  id bigint, owner uuid, kind text, model text, name text, author text,
  sha1 text, visibility text, created_at timestamptz, mine boolean
) language sql stable security definer set search_path = public as $$
  select t.id, t.owner, t.kind, t.model, t.name, t.author, t.sha1, t.visibility,
         t.created_at, t.owner = auth.uid()
    from shared_textures t
   where can_see_texture(t)
     and not exists (select 1 from texture_reports r
                      where r.texture_id = t.id and r.reporter = auth.uid())
   order by t.owner = auth.uid() desc, t.created_at desc
   limit least(greatest(coalesce(max_rows, 300), 1), 500);
$$;

create or replace function public.report_texture(texture_id bigint, reason text, note text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  t shared_textures;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  select * into t from shared_textures s where s.id = texture_id;
  if t.id is null or not can_see_texture(t) then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  if t.owner = me then
    raise exception 'textures.ownReport' using errcode = 'P0001';
  end if;
  if (select count(*) from texture_reports r
       where r.reporter = me and r.created_at > now() - interval '1 hour') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into texture_reports (texture_id, reporter, reason, note)
  values (texture_id, me, reason, nullif(btrim(coalesce(note, '')), ''))
  on conflict do nothing;
end;
$$;

-- Storage read check: `<owner>/<sha1>.png` of a share the caller can see.
create or replace function public.can_read_texture_file(path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from shared_textures t
     where t.owner::text || '/' || t.sha1 || '.png' = path and can_see_texture(t)
  );
$$;

revoke all on function public.can_see_texture(public.shared_textures) from public, anon, authenticated;
grant execute on function
  public.share_texture(text, text, text, text, text, text),
  public.unshare_texture(bigint),
  public.community_textures(int),
  public.report_texture(bigint, text, text),
  public.can_read_texture_file(text)
to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('textures', 'textures', false, 131072, array['image/png'])
on conflict (id) do update
  set public = false, file_size_limit = 131072, allowed_mime_types = array['image/png'];

drop policy if exists textures_upload on storage.objects;
create policy textures_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_update on storage.objects;
create policy textures_update on storage.objects for update to authenticated
  using (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_delete on storage.objects;
create policy textures_delete on storage.objects for delete to authenticated
  using (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_read on storage.objects;
create policy textures_read on storage.objects for select to authenticated
  using (bucket_id = 'textures' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.can_read_texture_file(name)));

-- ---------------------------------------------------------------- phase 19: MehburMC Library (K72)

-- One row per uploaded mod. The jar lives in the private `library` bucket as
-- <owner>/<sha1>.jar and is uploaded before the row is created. Rows start
-- as 'pending'; admins (phase 20) approve or reject them.
create table if not exists public.library_mods (
  id bigint generated always as identity primary key,
  owner uuid not null references auth.users (id) on delete cascade,
  author text not null check (author ~ '^[A-Za-z0-9_]{3,16}$'),
  name text not null check (char_length(btrim(name)) between 1 and 64),
  description text not null default '' check (char_length(description) <= 1000),
  mod_id text not null check (mod_id ~ '^[a-z0-9_.\-]{1,64}$'),
  version text not null check (char_length(version) between 1 and 64),
  loaders text[] not null check (
    cardinality(loaders) between 1 and 4
    and loaders <@ array['fabric', 'quilt', 'forge', 'neoforge']),
  game_versions text not null default '' check (char_length(game_versions) <= 100),
  sha1 text not null check (sha1 ~ '^[0-9a-f]{40}$'),
  size bigint not null check (size between 1 and 26214400),
  file_name text not null check (file_name ~ '^[A-Za-z0-9._+\-]{1,120}\.jar$'),
  status text not null default 'pending' check (status in ('pending', 'approved', 'rejected')),
  -- The uploader's automatic check (verdict 'pass' or 'warn', findings).
  scan jsonb not null,
  review_note text check (char_length(review_note) <= 500),
  reviewed_by uuid references auth.users (id) on delete set null,
  reviewed_at timestamptz,
  created_at timestamptz not null default now(),
  constraint library_mods_owner_sha1 unique (owner, sha1)
);
create index if not exists library_mods_status on public.library_mods (status, created_at desc);

create table if not exists public.library_reports (
  id bigint generated always as identity primary key,
  library_mod bigint not null references public.library_mods (id) on delete cascade,
  reporter uuid not null references auth.users (id) on delete cascade,
  reason text not null check (reason in ('malware', 'broken', 'stolen', 'other')),
  note text check (char_length(note) <= 500),
  created_at timestamptz not null default now(),
  unique (library_mod, reporter)
);

alter table public.library_mods enable row level security;
alter table public.library_reports enable row level security;
revoke all on public.library_mods, public.library_reports from anon, authenticated;

-- Submits (or resubmits) the caller's mod for review. Returns the row id.
create or replace function public.submit_library_mod(
  p_name text, p_description text, p_author text, p_mod_id text, p_version text,
  p_loaders text[], p_game_versions text, p_sha1 text, p_size bigint,
  p_file_name text, p_scan jsonb
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(p_sha1, ''));
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(p_author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  if coalesce(p_scan ->> 'verdict', '') not in ('pass', 'warn') then
    raise exception 'library.blocked' using errcode = 'P0001';
  end if;
  if not exists (select 1 from storage.objects o
                  where o.bucket_id = 'library' and o.name = me::text || '/' || clean_sha || '.jar') then
    raise exception 'library.fileMissing' using errcode = 'P0001';
  end if;
  select m.id into existing from library_mods m where m.owner = me and m.sha1 = clean_sha;
  if existing is null then
    if (select count(*) from library_mods m where m.owner = me and m.status = 'pending') >= 10 then
      raise exception 'library.quota' using errcode = 'P0001';
    end if;
    if (select count(*) from library_mods m
         where m.owner = me and m.created_at > now() - interval '1 day') >= 10 then
      raise exception 'friends.rateLimited' using errcode = 'P0001';
    end if;
  end if;
  insert into library_mods as m (owner, author, name, description, mod_id, version, loaders,
                                 game_versions, sha1, size, file_name, scan)
  values (me, btrim(p_author), btrim(p_name), btrim(coalesce(p_description, '')), p_mod_id,
          btrim(p_version), p_loaders, coalesce(p_game_versions, ''), clean_sha, p_size,
          p_file_name, p_scan)
  on conflict on constraint library_mods_owner_sha1 do update
    set author = excluded.author, name = excluded.name, description = excluded.description,
        scan = excluded.scan, status = 'pending', review_note = null,
        reviewed_by = null, reviewed_at = null
  returning m.id into out_id;
  return out_id;
end;
$$;

-- Withdraws one of the caller's uploads; returns its SHA-1 so the launcher
-- can delete the file.
create or replace function public.withdraw_library_mod(p_id bigint)
returns text language plpgsql security definer set search_path = public as $$
declare out_sha text;
begin
  delete from library_mods m where m.id = p_id and m.owner = auth.uid()
  returning m.sha1 into out_sha;
  if out_sha is null then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  return out_sha;
end;
$$;

-- Approved mods (minus the ones the caller reported) and all of the
-- caller's own uploads.
create or replace function public.library_list(p_max int default 300)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, mine boolean
) language sql stable security definer set search_path = public as $$
  select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
         m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan,
         case when m.owner = auth.uid() then m.review_note end,
         m.created_at, m.owner = auth.uid()
    from library_mods m
   where m.owner = auth.uid()
      or (m.status = 'approved'
          and not exists (select 1 from library_reports r
                           where r.library_mod = m.id and r.reporter = auth.uid()))
   order by m.owner = auth.uid() desc, m.created_at desc
   limit least(greatest(coalesce(p_max, 300), 1), 500);
$$;

create or replace function public.report_library_mod(p_id bigint, p_reason text, p_note text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  m library_mods;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  select * into m from library_mods x where x.id = p_id;
  if m.id is null or m.status <> 'approved' then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  if m.owner = me then
    raise exception 'textures.ownReport' using errcode = 'P0001';
  end if;
  if (select count(*) from library_reports r
       where r.reporter = me and r.created_at > now() - interval '1 hour') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into library_reports (library_mod, reporter, reason, note)
  values (p_id, me, p_reason, nullif(btrim(coalesce(p_note, '')), ''))
  on conflict do nothing;
end;
$$;

-- Storage read check: own files and files of approved mods.
create or replace function public.can_read_library_file(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from library_mods m
     where m.owner::text || '/' || m.sha1 || '.jar' = p_path
       and (m.owner = auth.uid() or m.status = 'approved')
  );
$$;

grant execute on function
  public.submit_library_mod(text, text, text, text, text, text[], text, text, bigint, text, jsonb),
  public.withdraw_library_mod(bigint),
  public.library_list(int),
  public.report_library_mod(bigint, text, text),
  public.can_read_library_file(text)
to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('library', 'library', false, 26214400, array['application/java-archive'])
on conflict (id) do update
  set public = false, file_size_limit = 26214400,
      allowed_mime_types = array['application/java-archive'];

drop policy if exists library_upload on storage.objects;
create policy library_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_update on storage.objects;
create policy library_update on storage.objects for update to authenticated
  using (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_delete on storage.objects;
create policy library_delete on storage.objects for delete to authenticated
  using (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_read on storage.objects;
create policy library_read on storage.objects for select to authenticated
  using (bucket_id = 'library' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.can_read_library_file(name)));
