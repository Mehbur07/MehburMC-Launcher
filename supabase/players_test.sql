-- MehburMC Launcher — checks for players.sql. Run in Supabase → SQL Editor
-- after players.sql. It creates throwaway users, acts as each of them and
-- always ends with an error so that nothing is saved:
--   "PLAYERS_OK ..."   → all checks passed
--   "FAIL: ..."        → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 5);
  founder uuid := gen_random_uuid();
  modr uuid := gen_random_uuid();
  alice uuid := gen_random_uuid();
  anon_u uuid := gen_random_uuid();
  r record;
  n int;
begin
  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (founder, 'plf' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (modr,    'plm' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (alice,   'pla' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (anon_u,  null,                            'authenticated', 'authenticated', true,  now(), now());
  insert into profiles (id, friend_code, display_name, last_seen) values
    (alice, 'PLA-' || upper(sfx), 'Alice', now());
  insert into account_names (name_lower, name, owner) values
    ('plf_' || sfx, 'Plf_' || sfx, founder),
    ('plm_' || sfx, 'Plm_' || sfx, modr),
    ('pla_' || sfx, 'Pla_' || sfx, alice),
    ('pln_' || sfx, 'Pln_' || sfx, anon_u);
  insert into admins (user_id, rank) values (founder, 1), (modr, 2);
  insert into bans (user_id, until, reason, banned_by) values (alice, now() + interval '1 day', 't', founder);

  -- ---------------------------------------------------------------- plain member: forbidden
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform * from public.admin_all_users('', 0, 10);
    raise exception 'FAIL: member listed players';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: member: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- moderator (rank 2) can list
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.admin_all_users('pla_' || sfx, 0, 10);
  if r.user_id is distinct from alice or not r.banned or not r.online
     or r.friend_code <> 'PLA-' || upper(sfx) or r.names <> array['Pla_' || sfx] or r.total <> 1 then
    raise exception 'FAIL: alice row %', r;
  end if;
  select count(*) into n from public.admin_all_users('PLA-' || upper(sfx), 0, 10);
  if n <> 1 then raise exception 'FAIL: friend code filter %', n; end if;
  select count(*) into n from public.admin_all_users('pln_' || sfx, 0, 10);
  if n <> 0 then raise exception 'FAIL: anonymous user listed'; end if;
  select count(*) into n from public.admin_all_users('%', 0, 10);
  if n <> 0 then raise exception 'FAIL: wildcard not escaped (% rows)', n; end if;

  -- ---------------------------------------------------------------- founder: all + paging
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  select * into r from public.admin_all_users('', 0, 1);
  if r.total < 3 then raise exception 'FAIL: total %', r.total; end if;
  -- Online players come first: no offline row may precede an online one.
  n := 0;
  for r in select * from public.admin_all_users('', 0, 200) loop
    if r.online and n > 0 then raise exception 'FAIL: online player after an offline one'; end if;
    if not r.online then n := n + 1; end if;
  end loop;
  select count(*) into n from public.admin_all_users('', 0, 1000);
  if n > 200 then raise exception 'FAIL: limit not capped (%)', n; end if;
  select count(*) into n from public.admin_all_users('', 1, 1);
  if n <> 1 then raise exception 'FAIL: offset page empty'; end if;
  select * into r from public.admin_all_users('plf_' || sfx, 0, 10);
  if r.rank <> 1 or r.banned then raise exception 'FAIL: founder row %', r; end if;
  execute 'reset role';

  raise exception 'PLAYERS_OK all checks passed (nothing was saved)';
end;
$test$;
