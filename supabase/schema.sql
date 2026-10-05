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
