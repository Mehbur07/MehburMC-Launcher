-- MehburMC Launcher — phase 23 (K85): nicknames for friends and group chats.
-- Run in Supabase → SQL Editor, then phase23_test.sql. Safe to run again.
--
-- Nicknames: each user may name a friend for themselves only; the friend
-- never sees it. Groups: up to 16 members; the owner renames, adds friends
-- and removes members; anyone leaves. Members must be email accounts that
-- are not banned. A group is deleted with its last member.

-- ---------------------------------------------------------------- nicknames

create table if not exists public.friend_nicknames (
  owner uuid not null references public.profiles (id) on delete cascade,
  friend uuid not null references public.profiles (id) on delete cascade,
  nickname text not null check (char_length(nickname) between 1 and 32),
  updated_at timestamptz not null default now(),
  primary key (owner, friend)
);
alter table public.friend_nicknames enable row level security;
-- RPC only: no policies.

-- Empty or null removes the nickname.
create or replace function public.set_nickname(p_friend uuid, p_nickname text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean text := left(btrim(regexp_replace(coalesce(p_nickname, ''), '\s+', ' ', 'g')), 32);
begin
  if not public.are_friends(me, p_friend) then
    raise exception 'friends.notFriends' using errcode = 'P0001';
  end if;
  if clean = '' then
    delete from friend_nicknames n where n.owner = me and n.friend = p_friend;
  else
    insert into friend_nicknames as n (owner, friend, nickname) values (me, p_friend, clean)
    on conflict on constraint friend_nicknames_pkey
    do update set nickname = excluded.nickname, updated_at = now();
  end if;
end;
$$;

-- my_friends() gains `nickname` (the caller's own name for the friend).
drop function if exists public.my_friends();
create function public.my_friends()
returns table (
  id uuid, friend_code text, display_name text, status text,
  incoming boolean, request_id bigint, unread bigint, blocked_by_me boolean,
  avatar_sha1 text, online boolean, nickname text
) language sql stable security definer set search_path = public as $$
  select p.id, p.friend_code, p.display_name, f.status,
         f.addressee = auth.uid() as incoming, f.id,
         (select count(*) from messages m
           where m.sender = p.id and m.recipient = auth.uid() and m.read_at is null),
         coalesce(f.blocked_by = auth.uid(), false),
         case when f.status = 'accepted' then p.avatar_sha1 end,
         f.status = 'accepted' and coalesce(p.last_seen > now() - interval '90 seconds', false),
         case when f.status = 'accepted' then
           (select n.nickname from friend_nicknames n where n.owner = auth.uid() and n.friend = p.id)
         end
    from friendships f
    join profiles p on p.id = case when f.requester = auth.uid() then f.addressee else f.requester end
   where (f.requester = auth.uid() or f.addressee = auth.uid())
     -- The blocked side does not see the relationship at all.
     and (f.status <> 'blocked' or f.blocked_by = auth.uid())
   order by p.display_name;
$$;

-- ---------------------------------------------------------------- groups

create table if not exists public.chat_groups (
  id bigint generated always as identity primary key,
  name text not null check (char_length(name) between 1 and 48),
  -- Null once the owner's profile is gone; group_owner() then falls back
  -- to the longest-standing member.
  owner uuid references public.profiles (id) on delete set null,
  created_at timestamptz not null default now()
);

create table if not exists public.group_members (
  group_id bigint not null references public.chat_groups (id) on delete cascade,
  member uuid not null references public.profiles (id) on delete cascade,
  -- clock_timestamp: members added in one call still get an order.
  joined_at timestamptz not null default clock_timestamp(),
  -- Highest group message id the member has read.
  last_read bigint not null default 0,
  primary key (group_id, member)
);
create index if not exists group_members_member on public.group_members (member);

create table if not exists public.group_messages (
  id bigint generated always as identity primary key,
  group_id bigint not null references public.chat_groups (id) on delete cascade,
  sender uuid not null references public.profiles (id) on delete cascade,
  body text not null check (char_length(body) between 1 and 2000),
  created_at timestamptz not null default now()
);
create index if not exists group_messages_group on public.group_messages (group_id, id);

create or replace function public.is_group_member(p_group bigint, p_user uuid)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (select 1 from group_members g where g.group_id = p_group and g.member = p_user);
$$;

create or replace function public.group_owner(p_group bigint)
returns uuid language sql stable security definer set search_path = public as $$
  select coalesce(
    (select c.owner from chat_groups c where c.id = p_group
        and exists (select 1 from group_members g where g.group_id = p_group and g.member = c.owner)),
    (select g.member from group_members g where g.group_id = p_group
      order by g.joined_at, g.member limit 1));
$$;

-- RLS helpers for the caller (granted; the general ones above are not).
create or replace function public.group_can_read(p_group bigint)
returns boolean language sql stable security definer set search_path = public as $$
  select public.is_group_member(p_group, auth.uid());
$$;
create or replace function public.group_can_send(p_group bigint)
returns boolean language sql stable security definer set search_path = public as $$
  select public.is_member() and not public.is_banned(auth.uid())
     and public.is_group_member(p_group, auth.uid());
$$;

-- An email account that is not banned (anonymous identities cannot chat in groups).
create or replace function public.require_chat_member()
returns uuid language plpgsql stable security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if me is null or not public.is_member() or public.is_banned(me)
     or not exists (select 1 from profiles p where p.id = me) then
    raise exception 'friends.disabled' using errcode = 'P0001';
  end if;
  return me;
end;
$$;

create or replace function public.group_name_ok(p_name text)
returns text language plpgsql immutable as $$
declare clean text := btrim(regexp_replace(coalesce(p_name, ''), '\s+', ' ', 'g'));
begin
  if char_length(clean) not between 1 and 48 then
    raise exception 'friends.groupNameInvalid' using errcode = 'P0001';
  end if;
  return clean;
end;
$$;

-- Adds friends of the caller; the group may hold at most 16 members.
create or replace function public.add_group_members_internal(p_group bigint, p_by uuid, p_members uuid[])
returns void language plpgsql security definer set search_path = public as $$
declare m uuid;
begin
  foreach m in array coalesce(p_members, '{}') loop
    continue when m = p_by or public.is_group_member(p_group, m);
    if not public.are_friends(p_by, m) then
      raise exception 'friends.notFriends' using errcode = 'P0001';
    end if;
    if exists (select 1 from friendships f
                where least(f.requester, f.addressee) = least(p_by, m)
                  and greatest(f.requester, f.addressee) = greatest(p_by, m)
                  and f.status = 'blocked') then
      raise exception 'friends.notFriends' using errcode = 'P0001';
    end if;
    insert into group_members (group_id, member, last_read)
    values (p_group, m, coalesce((select max(x.id) from group_messages x where x.group_id = p_group), 0));
  end loop;
  if (select count(*) from group_members g where g.group_id = p_group) > 16 then
    raise exception 'friends.groupFull' using errcode = 'P0001';
  end if;
end;
$$;

create or replace function public.create_group(p_name text, p_members uuid[])
returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_chat_member();
  gid bigint;
begin
  if (select count(*) from chat_groups c where c.owner = me) >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into chat_groups (name, owner) values (public.group_name_ok(p_name), me) returning id into gid;
  insert into group_members (group_id, member) values (gid, me);
  perform public.add_group_members_internal(gid, me, p_members);
  return gid;
end;
$$;

create or replace function public.require_group_owner(p_group bigint)
returns uuid language plpgsql stable security definer set search_path = public as $$
declare me uuid := public.require_chat_member();
begin
  if not exists (select 1 from chat_groups c where c.id = p_group) then
    raise exception 'friends.groupNotFound' using errcode = 'P0001';
  end if;
  if public.group_owner(p_group) is distinct from me then
    raise exception 'friends.notOwner' using errcode = 'P0001';
  end if;
  return me;
end;
$$;

create or replace function public.rename_group(p_group bigint, p_name text)
returns void language plpgsql security definer set search_path = public as $$
begin
  perform public.require_group_owner(p_group);
  update chat_groups c set name = public.group_name_ok(p_name) where c.id = p_group;
end;
$$;

create or replace function public.add_group_members(p_group bigint, p_members uuid[])
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_group_owner(p_group);
begin
  perform public.add_group_members_internal(p_group, me, p_members);
end;
$$;

-- After any membership ends (leave, removal, a deleted profile): the last
-- member gone deletes the group; a departed owner hands it to the
-- longest-standing member.
create or replace function public.group_member_gone()
returns trigger language plpgsql security definer set search_path = public as $$
declare next_owner uuid;
begin
  select g.member into next_owner from group_members g
   where g.group_id = old.group_id order by g.joined_at, g.member limit 1;
  if next_owner is null then
    delete from chat_groups c where c.id = old.group_id;
  else
    update chat_groups c set owner = next_owner
     where c.id = old.group_id and (c.owner is null or c.owner = old.member);
  end if;
  return null;
end;
$$;
drop trigger if exists group_member_gone on public.group_members;
create trigger group_member_gone after delete on public.group_members
  for each row execute function public.group_member_gone();

create or replace function public.drop_group_member(p_group bigint, p_member uuid)
returns void language sql security definer set search_path = public as $$
  delete from group_members g where g.group_id = p_group and g.member = p_member;
$$;

create or replace function public.remove_group_member(p_group bigint, p_member uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_group_owner(p_group);
begin
  if p_member = me then
    raise exception 'friends.notOwner' using errcode = 'P0001';
  end if;
  perform public.drop_group_member(p_group, p_member);
end;
$$;

create or replace function public.leave_group(p_group bigint)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  if not public.is_group_member(p_group, me) then
    raise exception 'friends.groupNotFound' using errcode = 'P0001';
  end if;
  perform public.drop_group_member(p_group, me);
end;
$$;

-- Groups of the caller with members (names; the caller's nicknames for
-- friends) and unread counts, most recently active first.
create or replace function public.my_groups()
returns table (
  id bigint, name text, owner uuid, members jsonb, unread bigint, last_message_at timestamptz
) language sql stable security definer set search_path = public as $$
  select c.id, c.name, public.group_owner(c.id),
         (select coalesce(jsonb_agg(jsonb_build_object(
                    'id', p.id,
                    'displayName', p.display_name,
                    'nickname', (select n.nickname from friend_nicknames n
                                  where n.owner = auth.uid() and n.friend = p.id),
                    'online', coalesce(p.last_seen > now() - interval '90 seconds', false))
                  order by g2.joined_at, p.id), '[]'::jsonb)
            from group_members g2 join profiles p on p.id = g2.member
           where g2.group_id = c.id),
         (select count(*) from group_messages x
           where x.group_id = c.id and x.id > g.last_read and x.sender <> auth.uid()),
         (select max(x.created_at) from group_messages x where x.group_id = c.id)
    from group_members g
    join chat_groups c on c.id = g.group_id
   where g.member = auth.uid()
   order by coalesce((select max(x.created_at) from group_messages x where x.group_id = c.id),
                     c.created_at) desc;
$$;

create or replace function public.mark_group_read(p_group bigint)
returns void language sql security definer set search_path = public as $$
  update group_members g
     set last_read = greatest(g.last_read,
                              coalesce((select max(x.id) from group_messages x where x.group_id = p_group), 0))
   where g.group_id = p_group and g.member = auth.uid();
$$;

create or replace function public.group_messages_rate_limit()
returns trigger language plpgsql security definer set search_path = public as $$
begin
  if (select count(*) from group_messages
       where sender = new.sender and created_at > now() - interval '1 minute') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  return new;
end;
$$;
drop trigger if exists group_messages_rate_limit on public.group_messages;
create trigger group_messages_rate_limit before insert on public.group_messages
  for each row execute function public.group_messages_rate_limit();

alter table public.chat_groups enable row level security;
alter table public.group_members enable row level security;
alter table public.group_messages enable row level security;
-- chat_groups and group_members change and are read only through the RPCs.

drop policy if exists group_messages_read on public.group_messages;
create policy group_messages_read on public.group_messages for select to authenticated
  using (public.group_can_read(group_id));
drop policy if exists group_messages_send on public.group_messages;
create policy group_messages_send on public.group_messages for insert to authenticated
  with check (sender = auth.uid() and public.group_can_send(group_id));

revoke all on function public.add_group_members_internal(bigint, uuid, uuid[]),
  public.drop_group_member(bigint, uuid), public.group_messages_rate_limit(),
  public.group_member_gone(), public.group_owner(bigint),
  public.require_chat_member(), public.require_group_owner(bigint),
  public.is_group_member(bigint, uuid), public.group_name_ok(text)
  from public, anon, authenticated;
grant execute on function public.set_nickname(uuid, text), public.my_friends(),
  public.create_group(text, uuid[]), public.rename_group(bigint, text),
  public.add_group_members(bigint, uuid[]), public.remove_group_member(bigint, uuid),
  public.leave_group(bigint), public.my_groups(), public.mark_group_read(bigint),
  public.group_can_read(bigint), public.group_can_send(bigint)
  to authenticated;
