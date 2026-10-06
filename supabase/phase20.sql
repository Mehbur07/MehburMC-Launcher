-- MehburMC Launcher — phase 20: email accounts, admins and bans
-- (ARCHITECTURE K73). Run once in Supabase → SQL Editor after phase19.sql.
-- Safe to re-run.
--
-- Accounts are Supabase Auth email + password users; the launcher upgrades
-- an existing anonymous identity in place, so its friends, names and shares
-- stay. Admin rank 1 is the founder (assigned by hand in the SQL editor),
-- rank 2 admins are appointed by rank 1. Admin power ("level"): rank 1 = 2,
-- rank 2 = 1, everyone else = 0. Every rule lives here; the launcher is
-- never trusted.

create table if not exists public.admins (
  user_id uuid primary key references auth.users (id) on delete cascade,
  rank smallint not null check (rank in (1, 2)),
  added_by uuid references auth.users (id) on delete set null,
  created_at timestamptz not null default now()
);

-- Active and expired bans; `until` null = permanent. The account cannot be
-- deleted while banned (see delete_me), so the row outlives sign-up tricks.
create table if not exists public.bans (
  user_id uuid primary key references auth.users (id) on delete cascade,
  until timestamptz,
  reason text not null default '' check (char_length(reason) <= 500),
  banned_by uuid references auth.users (id) on delete set null,
  created_at timestamptz not null default now()
);

create table if not exists public.admin_log (
  id bigint generated always as identity primary key,
  admin uuid references auth.users (id) on delete set null,
  action text not null,
  target text not null default '',
  detail text not null default '',
  created_at timestamptz not null default now()
);

alter table public.admins enable row level security;
alter table public.bans enable row level security;
alter table public.admin_log enable row level security;
revoke all on public.admins, public.bans, public.admin_log from anon, authenticated;

-- Admins re-check a mod in their own launcher before approving it.
alter table public.library_mods add column if not exists review_scan jsonb;

-- ---------------------------------------------------------------- helpers

create or replace function public.admin_level(p_user uuid)
returns int language sql stable security definer set search_path = public as $$
  select coalesce((select case a.rank when 1 then 2 else 1 end
                     from admins a where a.user_id = p_user), 0);
$$;

create or replace function public.is_banned(p_user uuid)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (select 1 from bans b
                  where b.user_id = p_user and (b.until is null or b.until > now()));
$$;

-- Signed in with an email account (not an anonymous identity).
create or replace function public.is_member()
returns boolean language sql stable set search_path = public as $$
  select auth.uid() is not null
     and coalesce((auth.jwt() ->> 'is_anonymous')::boolean, false) = false;
$$;

-- The caller, if they are an unbanned admin of at least `p_level`.
create or replace function public.require_admin(p_level int)
returns uuid language plpgsql stable security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if not public.is_member() or public.is_banned(me) or public.admin_level(me) < p_level then
    raise exception 'admin.forbidden' using errcode = 'P0001';
  end if;
  return me;
end;
$$;

create or replace function public.admin_note(p_admin uuid, p_action text, p_target text, p_detail text)
returns void language sql security definer set search_path = public as $$
  insert into admin_log (admin, action, target, detail)
  values (p_admin, p_action, coalesce(p_target, ''), left(coalesce(p_detail, ''), 500));
$$;

-- Account names of a user, for admin screens.
create or replace function public.user_names(p_user uuid)
returns text[] language sql stable security definer set search_path = public as $$
  select coalesce(array_agg(n.name order by n.created_at), '{}')
    from account_names n where n.owner = p_user;
$$;

revoke execute on function
  public.admin_level(uuid), public.is_banned(uuid), public.require_admin(int),
  public.admin_note(uuid, text, text, text), public.user_names(uuid)
from public, anon, authenticated;

-- ---------------------------------------------------------------- own status

-- The caller's admin rank (0 = none) and ban, checked by the launcher at
-- start and every few minutes.
create or replace function public.my_account()
returns table (rank int, banned boolean, ban_until timestamptz, ban_reason text)
language sql stable security definer set search_path = public as $$
  select coalesce((select a.rank::int from admins a where a.user_id = auth.uid()), 0),
         b.user_id is not null, b.until, b.reason
    from (select 1) one
    left join bans b on b.user_id = auth.uid() and (b.until is null or b.until > now());
$$;

-- An account cannot delete itself while banned (it would free the email).
create or replace function public.delete_me()
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if me is null then return; end if;
  if public.is_banned(me) then
    raise exception 'auth.banned' using errcode = 'P0001';
  end if;
  delete from profiles where id = me;
  delete from auth.users where id = me;
end;
$$;

-- ---------------------------------------------------------------- library

-- Library mods for admins: 'pending' (rank 1 only), 'approved' or
-- 'rejected', newest first, with their report count.
create or replace function public.admin_library(p_status text)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, reports bigint
) language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(case when p_status = 'pending' then 2 else 1 end);
  return query
    select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
           m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan, m.review_note,
           m.created_at,
           (select count(*) from library_reports r where r.library_mod = m.id)
      from library_mods m
     where m.status = p_status
     order by m.created_at desc
     limit 300;
end;
$$;

-- Rank 1 approves or rejects a mod. Approving needs the admin launcher's own
-- scan of the same file (`p_scan.sha1` must match, verdict pass or warn).
create or replace function public.admin_review_library_mod(
  p_id bigint, p_approve boolean, p_note text, p_scan jsonb
) returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(2);
  m library_mods;
begin
  select * into m from library_mods x where x.id = p_id;
  if m.id is null then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  if p_approve and (coalesce(p_scan ->> 'sha1', '') <> m.sha1
                    or coalesce(p_scan ->> 'verdict', '') not in ('pass', 'warn')) then
    raise exception 'library.blocked' using errcode = 'P0001';
  end if;
  update library_mods x
     set status = case when p_approve then 'approved' else 'rejected' end,
         review_note = nullif(left(btrim(coalesce(p_note, '')), 500), ''),
         review_scan = case when p_approve then p_scan else x.review_scan end,
         reviewed_by = me, reviewed_at = now()
   where x.id = p_id;
  perform public.admin_note(me, case when p_approve then 'approveMod' else 'rejectMod' end,
                            p_id::text, m.name);
end;
$$;

-- Any admin takes a published mod down (it becomes 'rejected').
create or replace function public.admin_remove_library_mod(p_id bigint, p_note text)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(1);
begin
  update library_mods x
     set status = 'rejected', review_note = nullif(left(btrim(coalesce(p_note, '')), 500), ''),
         reviewed_by = me, reviewed_at = now()
   where x.id = p_id and x.status = 'approved';
  if not found then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  perform public.admin_note(me, 'removeMod', p_id::text, p_note);
end;
$$;

-- Admins may download any library file (pending ones to review them).
create or replace function public.can_read_library_file(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from library_mods m
     where m.owner::text || '/' || m.sha1 || '.jar' = p_path
       and (m.owner = auth.uid() or m.status = 'approved'
            or (public.is_member() and public.admin_level(auth.uid()) >= 1
                and not public.is_banned(auth.uid())))
  );
$$;

-- ---------------------------------------------------------------- reports

-- Reported content still visible to users, most reported first. Library
-- mods for every admin, skins/capes for rank 1.
create or replace function public.admin_reports()
returns table (
  kind text, item_id bigint, name text, author text, owner uuid, sha1 text,
  reports bigint, reasons text[], notes text[], last_report timestamptz
) language plpgsql stable security definer set search_path = public as $$
declare lvl int;
begin
  perform public.require_admin(1);
  lvl := public.admin_level(auth.uid());
  return query
    select 'mod'::text, m.id, m.name, m.author, m.owner, m.sha1, count(r.id),
           array_agg(distinct r.reason),
           array_remove(array_agg(r.note order by r.created_at desc), null),
           max(r.created_at)
      from library_reports r join library_mods m on m.id = r.library_mod
     where m.status = 'approved'
     group by m.id
    union all
    select 'texture'::text, t.id, t.name, t.author, t.owner, t.sha1, count(r.id),
           array_agg(distinct r.reason),
           array_remove(array_agg(r.note order by r.created_at desc), null),
           max(r.created_at)
      from texture_reports r join shared_textures t on t.id = r.texture_id
     where not t.hidden and lvl >= 2
     group by t.id
     order by 7 desc, 10 desc
     limit 300;
end;
$$;

-- Clears the reports of an item without acting on it.
create or replace function public.admin_dismiss_reports(p_kind text, p_id bigint)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(case when p_kind = 'texture' then 2 else 1 end);
begin
  if p_kind = 'mod' then
    delete from library_reports r where r.library_mod = p_id;
  elsif p_kind = 'texture' then
    delete from texture_reports r where r.texture_id = p_id;
  else
    raise exception 'admin.forbidden' using errcode = 'P0001';
  end if;
  perform public.admin_note(me, 'dismissReports', p_kind || ':' || p_id, '');
end;
$$;

-- Rank 1 hides a shared skin/cape from everyone but its owner.
create or replace function public.admin_hide_texture(p_id bigint)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(2);
begin
  update shared_textures t set hidden = true where t.id = p_id;
  if not found then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  perform public.admin_note(me, 'hideTexture', p_id::text, '');
end;
$$;

-- ---------------------------------------------------------------- users & bans

-- Finds email accounts by account name (prefix) or friend code.
create or replace function public.admin_find_users(p_query text)
returns table (user_id uuid, names text[], rank int, banned boolean, ban_until timestamptz)
language plpgsql stable security definer set search_path = public as $$
declare q text := lower(btrim(coalesce(p_query, '')));
begin
  perform public.require_admin(1);
  if char_length(q) < 2 then return; end if;
  return query
    with hits as (
      select n.owner as uid from account_names n
       where n.name_lower like replace(replace(q, '_', '\_'), '%', '\%') || '%'
      union
      select p.id from profiles p where lower(p.friend_code) = q
    )
    select u.id, public.user_names(u.id),
           coalesce((select a.rank::int from admins a where a.user_id = u.id), 0),
           public.is_banned(u.id),
           (select b.until from bans b where b.user_id = u.id)
      from hits h join auth.users u on u.id = h.uid
     where not coalesce(u.is_anonymous, false)
     limit 20;
end;
$$;

-- Bans an account for `p_hours` (null = permanent; the server's clock
-- decides). Admins cannot be banned (rank 1 removes the rank first).
create or replace function public.admin_ban(p_user uuid, p_hours int, p_reason text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(1);
  p_until timestamptz;
begin
  if p_user = me then
    raise exception 'admin.self' using errcode = 'P0001';
  end if;
  if public.admin_level(p_user) > 0 then
    raise exception 'admin.targetAdmin' using errcode = 'P0001';
  end if;
  if not exists (select 1 from auth.users u where u.id = p_user) then
    raise exception 'admin.userNotFound' using errcode = 'P0001';
  end if;
  if p_hours is not null and (p_hours < 1 or p_hours > 24 * 365 * 10) then
    raise exception 'admin.badUntil' using errcode = 'P0001';
  end if;
  p_until := case when p_hours is null then null else now() + make_interval(hours => p_hours) end;
  insert into bans as b (user_id, until, reason, banned_by)
  values (p_user, p_until, left(btrim(coalesce(p_reason, '')), 500), me)
  on conflict (user_id) do update
    set until = excluded.until, reason = excluded.reason,
        banned_by = excluded.banned_by, created_at = now();
  -- Supabase Auth refuses sign-in and token refresh while banned_until is
  -- in the future ("permanent" = 100 years).
  update auth.users u
     set banned_until = coalesce(p_until, now() + interval '100 years')
   where u.id = p_user;
  update profiles p set last_seen = null where p.id = p_user;
  perform public.admin_note(me, 'ban', p_user::text,
                            coalesce(p_until::text, 'permanent') || ' ' || coalesce(p_reason, ''));
end;
$$;

create or replace function public.admin_unban(p_user uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(1);
begin
  delete from bans b where b.user_id = p_user;
  update auth.users u set banned_until = null where u.id = p_user;
  perform public.admin_note(me, 'unban', p_user::text, '');
end;
$$;

create or replace function public.admin_bans()
returns table (user_id uuid, names text[], until timestamptz, reason text,
               banned_by text[], created_at timestamptz, active boolean)
language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(1);
  return query
    select b.user_id, public.user_names(b.user_id), b.until, b.reason,
           case when b.banned_by is null then '{}'::text[] else public.user_names(b.banned_by) end,
           b.created_at, (b.until is null or b.until > now())
      from bans b
     order by (b.until is null or b.until > now()) desc, b.created_at desc
     limit 500;
end;
$$;

-- ---------------------------------------------------------------- admins

create or replace function public.admin_list()
returns table (user_id uuid, names text[], rank int, created_at timestamptz)
language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(1);
  return query
    select a.user_id, public.user_names(a.user_id), a.rank::int, a.created_at
      from admins a order by a.rank, a.created_at;
end;
$$;

-- Rank 1 appoints (p_rank = 2) or removes (p_rank = 0) a rank 2 admin.
create or replace function public.admin_set_rank(p_user uuid, p_rank int)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(2);
begin
  if p_user = me or public.admin_level(p_user) = 2 then
    raise exception 'admin.targetAdmin' using errcode = 'P0001';
  end if;
  if p_rank = 0 then
    delete from admins a where a.user_id = p_user;
  elsif p_rank = 2 then
    if not exists (select 1 from auth.users u
                    where u.id = p_user and not coalesce(u.is_anonymous, false)) then
      raise exception 'admin.userNotFound' using errcode = 'P0001';
    end if;
    if public.is_banned(p_user) then
      raise exception 'auth.banned' using errcode = 'P0001';
    end if;
    insert into admins (user_id, rank, added_by) values (p_user, 2, me)
    on conflict (user_id) do nothing;
  else
    raise exception 'admin.forbidden' using errcode = 'P0001';
  end if;
  perform public.admin_note(me, 'setRank', p_user::text, p_rank::text);
end;
$$;

grant execute on function
  public.my_account(),
  public.delete_me(),
  public.admin_library(text),
  public.admin_review_library_mod(bigint, boolean, text, jsonb),
  public.admin_remove_library_mod(bigint, text),
  public.can_read_library_file(text),
  public.admin_reports(),
  public.admin_dismiss_reports(text, bigint),
  public.admin_hide_texture(bigint),
  public.admin_find_users(text),
  public.admin_ban(uuid, int, text),
  public.admin_unban(uuid),
  public.admin_bans(),
  public.admin_list(),
  public.admin_set_rank(uuid, int)
to authenticated;
