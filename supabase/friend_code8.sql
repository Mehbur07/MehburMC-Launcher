-- MehburMC Launcher — friend codes grow from 4 to 8 characters (K100). Run in
-- Supabase → SQL Editor, then friend_code8_test.sql. Safe to run more than once.
--
-- Existing codes keep their 4 characters and gain 4 random ones, so the old
-- short code stops working. Friendships use profile ids and are not touched.

-- 'MEHBUR-' + 8 characters without look-alikes (no 0/O, 1/I/L).
create or replace function public.new_friend_code()
returns text language plpgsql volatile set search_path = public as $$
declare
  alphabet constant text := '23456789ABCDEFGHJKMNPQRSTUVWXYZ';
  code text;
begin
  loop
    code := 'MEHBUR-';
    for i in 1..8 loop
      code := code || substr(alphabet, 1 + floor(random() * length(alphabet))::int, 1);
    end loop;
    exit when not exists (select 1 from profiles where friend_code = code);
  end loop;
  return code;
end;
$$;
revoke all on function public.new_friend_code() from public, anon, authenticated;

do $$
declare
  r record;
begin
  for r in select id, friend_code from public.profiles
            where friend_code ~ '^MEHBUR-[A-Z0-9]{4}$' loop
    -- The characters of a fresh code: 'MEHBUR-' is 7 long.
    update public.profiles
       set friend_code = r.friend_code || substr(public.new_friend_code(), 8, 4)
     where id = r.id;
  end loop;
end;
$$;
