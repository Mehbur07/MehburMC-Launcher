-- MehburMC Launcher — checks for friend_code8.sql. Run in Supabase → SQL Editor
-- after friend_code8.sql. It creates throwaway users, acts as each of them and
-- always ends with an error so that nothing is saved:
--   "FRIENDCODE8_OK ..." → all checks passed
--   "FAIL: ..."          → a rule is broken (send the message to the developer)

do $test$
declare
  a uuid := gen_random_uuid();
  b uuid := gen_random_uuid();
  pa profiles;
  pb profiles;
  c text;
  n int;
begin
  -- ---------------------------------------------------------------- every code on the server
  select count(*) into n from profiles where friend_code ~ '^MEHBUR-[A-Z0-9]{4}$';
  if n <> 0 then raise exception 'FAIL: % short codes left, run friend_code8.sql', n; end if;

  -- ---------------------------------------------------------------- the generator
  for i in 1..500 loop
    c := new_friend_code();
    if c !~ '^MEHBUR-[2-9A-HJKMNP-Z]{8}$' then raise exception 'FAIL: bad code %', c; end if;
  end loop;

  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at)
  select x, 'fc8' || left(x::text, 8) || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()
    from unnest(array[a, b]) x;

  -- ---------------------------------------------------------------- new accounts
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  pb := public.ensure_profile('Bob');
  execute 'reset role';
  if pb.friend_code !~ '^MEHBUR-[2-9A-HJKMNP-Z]{8}$' then
    raise exception 'FAIL: new account got %', pb.friend_code;
  end if;

  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  pa := public.ensure_profile('Alice');

  -- ---------------------------------------------------------------- adding by code
  begin
    perform public.send_request(left(pb.friend_code, 11));
    raise exception 'FAIL: the first 4 characters alone found a friend';
  exception when others then
    if sqlerrm <> 'friends.codeNotFound' then raise exception 'FAIL: short code: %', sqlerrm; end if;
  end;
  if public.send_request(' ' || lower(pb.friend_code) || ' ') <> 'pending' then
    raise exception 'FAIL: request by the 8 character code';
  end if;

  -- ---------------------------------------------------------------- the code stays on rename
  pa := public.ensure_profile('Alice 2');
  if pa.display_name <> 'Alice 2' or pa.friend_code <> (select friend_code from profiles where id = a) then
    raise exception 'FAIL: rename changed the code';
  end if;
  execute 'reset role';

  raise exception 'FRIENDCODE8_OK all checks passed (nothing was saved)';
end;
$test$;
