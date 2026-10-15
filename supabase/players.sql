-- MehburMC Launcher — admin player list (K80). Run in Supabase → SQL Editor,
-- then players_test.sql. Safe to run more than once.
--
-- Every admin (rank 1 and 2) can list all email accounts, newest activity
-- first, a page at a time, optionally filtered by a name part or an exact
-- friend code. Emails are never returned.

create or replace function public.admin_all_users(
  p_query text default '',
  p_offset int default 0,
  p_limit int default 100
)
returns table (
  user_id uuid,
  names text[],
  friend_code text,
  rank int,
  banned boolean,
  ban_until timestamptz,
  joined_at timestamptz,
  last_seen timestamptz,
  online boolean,
  total bigint
)
language plpgsql stable security definer set search_path = public as $$
declare
  q text := lower(btrim(coalesce(p_query, '')));
  pat text := '%' || replace(replace(replace(q, '\', '\\'), '_', '\_'), '%', '\%') || '%';
begin
  perform public.require_admin(1);
  return query
    with members as (
      select u.id as uid, u.created_at as joined, p.friend_code as code, p.last_seen as seen,
             coalesce(p.last_seen > now() - interval '90 seconds', false) as is_online
        from auth.users u
        left join profiles p on p.id = u.id
       where not coalesce(u.is_anonymous, false)
         and (q = ''
              or lower(p.friend_code) = q
              or exists (select 1 from account_names n
                          where n.owner = u.id and n.name_lower like pat))
    )
    select m.uid, public.user_names(m.uid), m.code,
           coalesce((select a.rank::int from admins a where a.user_id = m.uid), 0),
           public.is_banned(m.uid),
           (select b.until from bans b where b.user_id = m.uid),
           m.joined, m.seen, m.is_online,
           count(*) over ()
      from members m
     order by m.is_online desc, m.seen desc nulls last, m.joined desc, m.uid
     limit least(greatest(coalesce(p_limit, 100), 1), 200)
     offset greatest(coalesce(p_offset, 0), 0);
end;
$$;

revoke execute on function public.admin_all_users(text, int, int) from public, anon;
grant execute on function public.admin_all_users(text, int, int) to authenticated;
