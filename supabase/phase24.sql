-- MehburMC Launcher — phase 24 (K86): voice calls with a friend or a group.
-- Run in Supabase → SQL Editor after phase23.sql, then phase24_test.sql.
-- Safe to run again.
--
-- Audio flows peer to peer (WebRTC, optionally through Cloudflare TURN);
-- the server only keeps who is in which call and relays the connection
-- set-up messages ("signals"), which are deleted once delivered. A direct
-- call ends when either side leaves; a group call while anyone is in it.
-- Members who stop polling for 20 s count as gone; ringing stops after 45 s.

-- ---------------------------------------------------------------- tables

create table if not exists public.calls (
  id bigint generated always as identity primary key,
  -- Null for a call between two friends.
  group_id bigint references public.chat_groups (id) on delete cascade,
  created_by uuid references public.profiles (id) on delete set null,
  created_at timestamptz not null default now(),
  ended_at timestamptz
);
-- One live call per group.
create unique index if not exists calls_one_live_per_group
  on public.calls (group_id) where ended_at is null and group_id is not null;
create index if not exists calls_created_by on public.calls (created_by, created_at);

create table if not exists public.call_members (
  call_id bigint not null references public.calls (id) on delete cascade,
  member uuid not null references public.profiles (id) on delete cascade,
  state text not null check (state in ('ringing', 'joined', 'left', 'declined', 'missed')),
  invited_at timestamptz not null default now(),
  -- clock_timestamp: orders members who join in the same instant; the later
  -- joiner of each pair sends the offer.
  joined_at timestamptz,
  -- Last poll while joined.
  seen_at timestamptz not null default now(),
  primary key (call_id, member)
);
create index if not exists call_members_live on public.call_members (member)
  where state in ('ringing', 'joined');

create table if not exists public.call_signals (
  id bigint generated always as identity primary key,
  call_id bigint not null references public.calls (id) on delete cascade,
  sender uuid not null references public.profiles (id) on delete cascade,
  recipient uuid not null references public.profiles (id) on delete cascade,
  kind text not null check (kind in ('offer', 'answer', 'ice')),
  payload text not null check (char_length(payload) between 1 and 16000),
  created_at timestamptz not null default now()
);
create index if not exists call_signals_recipient on public.call_signals (recipient, id);
create index if not exists call_signals_sender on public.call_signals (sender, created_at);

alter table public.calls enable row level security;
alter table public.call_members enable row level security;
alter table public.call_signals enable row level security;
-- RPC only: no policies.

-- ---------------------------------------------------------------- internals

-- Ends a call: whoever is still ringing missed it, the rest left.
create or replace function public.call_end(p_call bigint)
returns void language sql security definer set search_path = public as $$
  update calls c set ended_at = now() where c.id = p_call and c.ended_at is null;
  update call_members m
     set state = case when m.state = 'ringing' then 'missed' else 'left' end
   where m.call_id = p_call and m.state in ('ringing', 'joined');
  delete from call_signals s where s.call_id = p_call;
$$;

-- Applies the time-outs and membership rules to one call.
create or replace function public.call_tidy(p_call bigint)
returns void language plpgsql security definer set search_path = public as $$
declare
  c calls%rowtype;
  pair uuid[];
begin
  select * into c from calls x where x.id = p_call for update;
  if not found or c.ended_at is not null then
    return;
  end if;
  -- Dropped without leaving (crash, lost connection).
  update call_members m set state = 'left'
   where m.call_id = p_call and m.state = 'joined' and m.seen_at < now() - interval '20 seconds';
  -- Nobody answered.
  update call_members m set state = 'missed'
   where m.call_id = p_call and m.state = 'ringing' and m.invited_at < now() - interval '45 seconds';
  if c.group_id is not null then
    -- Removed from the group (or the profile is gone): out of its call too.
    update call_members m
       set state = case when m.state = 'ringing' then 'missed' else 'left' end
     where m.call_id = p_call and m.state in ('ringing', 'joined')
       and not public.is_group_member(c.group_id, m.member);
    if not exists (select 1 from call_members m where m.call_id = p_call and m.state = 'joined') then
      perform public.call_end(p_call);
    end if;
  else
    select array_agg(m.member) into pair from call_members m where m.call_id = p_call;
    if (select count(*) from call_members m
         where m.call_id = p_call and m.state in ('ringing', 'joined')) < 2
       or coalesce(array_length(pair, 1), 0) <> 2
       or not public.are_friends(pair[1], pair[2]) then
      perform public.call_end(p_call);
    end if;
  end if;
end;
$$;

-- Leaves (or declines) every live call of `p_user` except `p_except`.
create or replace function public.call_drop_user(p_user uuid, p_except bigint)
returns void language plpgsql security definer set search_path = public as $$
declare cid bigint;
begin
  for cid in
    update call_members m
       set state = case when m.state = 'ringing' then 'declined' else 'left' end
     where m.member = p_user and m.state in ('ringing', 'joined')
       and m.call_id is distinct from p_except
    returning m.call_id
  loop
    delete from call_signals s where s.call_id = cid and (s.sender = p_user or s.recipient = p_user);
    perform public.call_tidy(cid);
  end loop;
end;
$$;

-- Live calls `p_user` rings in or is in, and the live calls of their groups.
create or replace function public.call_visible(p_user uuid)
returns setof bigint language sql stable security definer set search_path = public as $$
  select m.call_id from call_members m
    join calls c on c.id = m.call_id and c.ended_at is null
   where m.member = p_user and m.state in ('ringing', 'joined')
  union
  select c.id from group_members g
    join calls c on c.group_id = g.group_id and c.ended_at is null
   where g.member = p_user;
$$;

-- Old set-up messages and finished calls are not kept.
create or replace function public.call_cleanup()
returns void language sql security definer set search_path = public as $$
  delete from call_signals s where s.created_at < now() - interval '2 minutes';
  delete from calls c where c.ended_at < now() - interval '1 day';
$$;

-- ---------------------------------------------------------------- calls

-- Joins a live call the caller may be in (rings or is a group member).
create or replace function public.call_join(p_call bigint)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_chat_member();
  c calls%rowtype;
begin
  perform public.call_tidy(p_call);
  select * into c from calls x where x.id = p_call;
  if not found then
    raise exception 'calls.notFound' using errcode = 'P0001';
  end if;
  if c.ended_at is not null then
    raise exception 'calls.ended' using errcode = 'P0001';
  end if;
  if c.group_id is not null then
    if not public.is_group_member(c.group_id, me) then
      raise exception 'friends.groupNotFound' using errcode = 'P0001';
    end if;
    insert into call_members as m (call_id, member, state, joined_at, seen_at)
    values (p_call, me, 'joined', clock_timestamp(), now())
    on conflict on constraint call_members_pkey
    do update set state = 'joined', joined_at = clock_timestamp(), seen_at = now();
  else
    update call_members m set state = 'joined', joined_at = clock_timestamp(), seen_at = now()
     where m.call_id = p_call and m.member = me and m.state in ('ringing', 'joined');
    if not found then
      raise exception 'calls.notFound' using errcode = 'P0001';
    end if;
  end if;
  -- A fresh session: nothing from an earlier one is replayed.
  delete from call_signals s where s.call_id = p_call and (s.sender = me or s.recipient = me);
  perform public.call_drop_user(me, p_call);
end;
$$;

-- Calls a friend (p_friend) or a group (p_group); joins the live call
-- instead if there is one. Returns the call id.
create or replace function public.call_start(p_friend uuid default null, p_group bigint default null)
returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_chat_member();
  cid bigint;
begin
  if (p_friend is null) = (p_group is null) then
    raise exception 'calls.invalid' using errcode = 'P0001';
  end if;
  perform public.call_cleanup();

  if p_friend is not null then
    if p_friend = me or not public.are_friends(me, p_friend) then
      raise exception 'friends.notFriends' using errcode = 'P0001';
    end if;
    select c.id into cid from calls c
     where c.group_id is null and c.ended_at is null
       and exists (select 1 from call_members m where m.call_id = c.id and m.member = me)
       and exists (select 1 from call_members m where m.call_id = c.id and m.member = p_friend)
     order by c.id desc limit 1;
  else
    if not public.is_group_member(p_group, me) then
      raise exception 'friends.groupNotFound' using errcode = 'P0001';
    end if;
    select c.id into cid from calls c where c.group_id = p_group and c.ended_at is null;
  end if;
  if cid is not null then
    perform public.call_tidy(cid);
    if exists (select 1 from calls c where c.id = cid and c.ended_at is null) then
      perform public.call_join(cid);
      return cid;
    end if;
  end if;

  if (select count(*) from calls c
       where c.created_by = me and c.created_at > now() - interval '1 minute') >= 10 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into calls as c (group_id, created_by) values (p_group, me)
  on conflict (group_id) where ended_at is null and group_id is not null do nothing
  returning c.id into cid;
  if cid is null then
    -- Someone else started the group's call a moment ago.
    select c.id into cid from calls c where c.group_id = p_group and c.ended_at is null;
    perform public.call_join(cid);
    return cid;
  end if;
  insert into call_members (call_id, member, state, joined_at)
  values (cid, me, 'joined', clock_timestamp());
  if p_friend is not null then
    insert into call_members (call_id, member, state) values (cid, p_friend, 'ringing');
  else
    insert into call_members (call_id, member, state)
    select cid, g.member, 'ringing' from group_members g
     where g.group_id = p_group and g.member <> me;
  end if;
  perform public.call_drop_user(me, cid);
  return cid;
end;
$$;

-- Leaves a call, or declines it while it rings.
create or replace function public.call_leave(p_call bigint)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := auth.uid();
begin
  update call_members m
     set state = case when m.state = 'ringing' then 'declined' else 'left' end
   where m.call_id = p_call and m.member = me and m.state in ('ringing', 'joined');
  delete from call_signals s where s.call_id = p_call and (s.sender = me or s.recipient = me);
  perform public.call_tidy(p_call);
end;
$$;

-- Connection set-up messages to other members of the call:
-- [{"recipient": uuid, "kind": "offer"|"answer"|"ice", "payload": text}, …]
-- Messages to members who are no longer in the call are dropped.
create or replace function public.call_signal(p_call bigint, p_signals jsonb)
returns int language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_chat_member();
  n int := coalesce(jsonb_array_length(case when jsonb_typeof(p_signals) = 'array' then p_signals end), -1);
  sent int;
begin
  if n < 1 or n > 50 then
    raise exception 'calls.invalid' using errcode = 'P0001';
  end if;
  if not exists (select 1 from call_members m join calls c on c.id = m.call_id
                  where m.call_id = p_call and m.member = me and m.state = 'joined'
                    and c.ended_at is null) then
    raise exception 'calls.ended' using errcode = 'P0001';
  end if;
  if exists (select 1 from jsonb_array_elements(p_signals) e(v)
              where jsonb_typeof(e.v) <> 'object'
                 or coalesce(e.v->>'recipient', '') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
                 or coalesce(e.v->>'kind', '') not in ('offer', 'answer', 'ice')
                 or char_length(coalesce(e.v->>'payload', '')) not between 1 and 16000) then
    raise exception 'calls.invalid' using errcode = 'P0001';
  end if;
  if (select count(*) from call_signals s
       where s.sender = me and s.created_at > now() - interval '1 minute') + n > 1200 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into call_signals (call_id, sender, recipient, kind, payload)
  select p_call, me, (e.v->>'recipient')::uuid, e.v->>'kind', e.v->>'payload'
    from jsonb_array_elements(p_signals) with ordinality as e(v, ord)
    join call_members m on m.call_id = p_call and m.member = (e.v->>'recipient')::uuid
                       and m.state = 'joined'
   where (e.v->>'recipient')::uuid <> me
   order by e.ord;
  get diagnostics sent = row_count;
  return sent;
end;
$$;

-- One round trip for the launcher's poll loop: marks the caller as still
-- there, applies time-outs, drops delivered signals (id <= p_after) and
-- returns the live calls the caller rings in, is in or could join (their
-- groups), plus the signals addressed to them.
create or replace function public.call_poll(p_after bigint default 0)
returns jsonb language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  cid bigint;
  out_calls jsonb;
  out_signals jsonb;
begin
  if me is null or not public.is_member() or public.is_banned(me) then
    return jsonb_build_object('calls', '[]'::jsonb, 'signals', '[]'::jsonb);
  end if;
  update call_members m set seen_at = now() where m.member = me and m.state = 'joined';
  for cid in select public.call_visible(me) order by 1
  loop
    perform public.call_tidy(cid);
  end loop;
  delete from call_signals s where s.recipient = me and s.id <= coalesce(p_after, 0);

  select coalesce(jsonb_agg(jsonb_build_object(
           'id', c.id,
           'groupId', c.group_id,
           'createdBy', c.created_by,
           'createdAt', c.created_at,
           'myState', (select m.state from call_members m where m.call_id = c.id and m.member = me),
           'members', (select coalesce(jsonb_agg(jsonb_build_object(
                          'id', p.id,
                          'displayName', p.display_name,
                          'nickname', (select n.nickname from friend_nicknames n
                                        where n.owner = me and n.friend = p.id),
                          'state', m.state,
                          'joinedAt', m.joined_at)
                        order by m.joined_at nulls last, p.id), '[]'::jsonb)
                         from call_members m join profiles p on p.id = m.member
                        where m.call_id = c.id and m.state in ('ringing', 'joined')))
         order by c.id), '[]'::jsonb)
    into out_calls
    from calls c
   where c.ended_at is null and c.id in (select public.call_visible(me));

  select coalesce(jsonb_agg(jsonb_build_object(
           'id', s.id, 'callId', s.call_id, 'sender', s.sender,
           'kind', s.kind, 'payload', s.payload) order by s.id), '[]'::jsonb)
    into out_signals
    from (select s.* from call_signals s
            join call_members m on m.call_id = s.call_id and m.member = me and m.state = 'joined'
           where s.recipient = me and s.id > coalesce(p_after, 0)
           order by s.id limit 200) s;

  return jsonb_build_object('calls', out_calls, 'signals', out_signals);
end;
$$;

revoke all on function public.call_end(bigint), public.call_tidy(bigint), public.call_visible(uuid),
  public.call_drop_user(uuid, bigint), public.call_cleanup()
  from public, anon, authenticated;
grant execute on function public.call_start(uuid, bigint), public.call_join(bigint),
  public.call_leave(bigint), public.call_signal(bigint, jsonb), public.call_poll(bigint)
  to authenticated;
