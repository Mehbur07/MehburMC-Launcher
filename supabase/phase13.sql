-- MehburMC Launcher — phase 13: online status for friends (ARCHITECTURE K66).
-- Run once in Supabase → SQL Editor after phase12.sql. Safe to re-run.

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
