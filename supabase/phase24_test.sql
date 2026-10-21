-- MehburMC Launcher — checks for phase24.sql. Run in Supabase → SQL Editor
-- after phase24.sql. It creates throwaway users, acts as each of them and
-- always ends with an error so that nothing is saved:
--   "PHASE24_OK ..."   → all checks passed
--   "FAIL: ..."        → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 4);
  a uuid := gen_random_uuid();
  b uuid := gen_random_uuid();
  c uuid := gen_random_uuid();
  d uuid := gen_random_uuid();
  gid bigint;
  c1 bigint;
  c2 bigint;
  c3 bigint;
  p jsonb;
  n int;
  sid bigint;
begin
  if to_regprocedure('public.call_poll(bigint)') is null then
    raise exception 'FAIL: run phase24.sql first (call_poll is missing)';
  end if;

  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at)
  select x, 'p24' || left(x::text, 8) || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()
    from unnest(array[a, b, c, d]) x;
  insert into profiles (id, friend_code, display_name) values
    (a, 'P24A-' || upper(sfx), 'Alice'), (b, 'P24B-' || upper(sfx), 'Bob'),
    (c, 'P24C-' || upper(sfx), 'Cem'), (d, 'P24D-' || upper(sfx), 'Deniz');
  insert into friendships (requester, addressee, status) values
    (a, b, 'accepted'), (a, c, 'accepted');

  -- ---------------------------------------------------------------- direct call (Alice → Bob)
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.call_start();
    raise exception 'FAIL: call without a target';
  exception when others then
    if sqlerrm <> 'calls.invalid' then raise exception 'FAIL: no target: %', sqlerrm; end if;
  end;
  begin
    perform public.call_start(p_friend => d);
    raise exception 'FAIL: called a non-friend';
  exception when others then
    if sqlerrm <> 'friends.notFriends' then raise exception 'FAIL: non-friend call: %', sqlerrm; end if;
  end;
  c1 := public.call_start(p_friend => b);
  if public.call_start(p_friend => b) <> c1 then raise exception 'FAIL: second call to the same friend'; end if;
  select count(*) into n from calls;
  if n <> 0 then raise exception 'FAIL: calls table readable'; end if;
  select count(*) into n from call_signals;
  if n <> 0 then raise exception 'FAIL: signals table readable'; end if;
  execute 'reset role';

  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  p := public.call_poll(0);
  if jsonb_array_length(p->'calls') <> 1 or p->'calls'->0->>'myState' <> 'ringing'
     or jsonb_array_length(p->'calls'->0->'members') <> 2 then
    raise exception 'FAIL: Bob does not see the ringing call %', p;
  end if;
  begin
    perform public.call_signal(c1, '[{"recipient": "00000000-0000-0000-0000-000000000000", "kind": "ice", "payload": "x"}]');
    raise exception 'FAIL: signalled before joining';
  exception when others then
    if sqlerrm <> 'calls.ended' then raise exception 'FAIL: signal before join: %', sqlerrm; end if;
  end;
  perform public.call_join(c1);
  execute 'reset role';

  -- Cem (a friend of Alice, not of this call) cannot join it.
  perform set_config('request.jwt.claims',
    json_build_object('sub', c, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.call_join(c1);
    raise exception 'FAIL: outsider joined a direct call';
  exception when others then
    if sqlerrm <> 'calls.notFound' then raise exception 'FAIL: outsider join: %', sqlerrm; end if;
  end;
  if jsonb_array_length(public.call_poll(0)->'calls') <> 0 then raise exception 'FAIL: outsider sees the call'; end if;
  execute 'reset role';

  -- Signals: delivered to joined members only, deleted once acknowledged.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  n := public.call_signal(c1, jsonb_build_array(
    jsonb_build_object('recipient', b, 'kind', 'offer', 'payload', 'sdp-1'),
    jsonb_build_object('recipient', b, 'kind', 'ice', 'payload', 'cand-1'),
    jsonb_build_object('recipient', c, 'kind', 'ice', 'payload', 'not in the call'),
    jsonb_build_object('recipient', a, 'kind', 'ice', 'payload', 'to myself')));
  if n <> 2 then raise exception 'FAIL: % signals sent, expected 2', n; end if;
  begin
    perform public.call_signal(c1, jsonb_build_array(jsonb_build_object('recipient', b, 'kind', 'bye', 'payload', 'x')));
    raise exception 'FAIL: unknown signal kind';
  exception when others then
    if sqlerrm <> 'calls.invalid' then raise exception 'FAIL: signal kind: %', sqlerrm; end if;
  end;
  begin
    perform public.call_signal(c1, jsonb_build_array(jsonb_build_object('recipient', b, 'kind', 'ice', 'payload', repeat('x', 16001))));
    raise exception 'FAIL: oversized signal';
  exception when others then
    if sqlerrm <> 'calls.invalid' then raise exception 'FAIL: signal size: %', sqlerrm; end if;
  end;
  execute 'reset role';

  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  p := public.call_poll(0);
  if jsonb_array_length(p->'signals') <> 2 or p->'signals'->0->>'kind' <> 'offer'
     or p->'signals'->0->>'payload' <> 'sdp-1' or p->'signals'->0->>'sender' <> a::text then
    raise exception 'FAIL: Bob got signals %', p->'signals';
  end if;
  if p->'calls'->0->>'myState' <> 'joined' then raise exception 'FAIL: Bob not joined %', p; end if;
  sid := (p->'signals'->1->>'id')::bigint;
  p := public.call_poll(sid);
  if jsonb_array_length(p->'signals') <> 0 then raise exception 'FAIL: signals delivered twice'; end if;
  execute 'reset role';
  select count(*) into n from call_signals s where s.call_id = c1;
  if n <> 0 then raise exception 'FAIL: acknowledged signals kept'; end if;

  -- Alice hangs up: the direct call ends for Bob too.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.call_leave(c1);
  execute 'reset role';
  if (select ended_at from calls where id = c1) is null then raise exception 'FAIL: direct call kept after hang-up'; end if;

  -- Declining a ringing call ends it as well.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  c2 := public.call_start(p_friend => b);
  if c2 = c1 then raise exception 'FAIL: ended call reused'; end if;
  execute 'reset role';
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.call_leave(c2);
  execute 'reset role';
  if (select state from call_members where call_id = c2 and member = b) <> 'declined'
     or (select ended_at from calls where id = c2) is null then
    raise exception 'FAIL: declined call';
  end if;

  -- Unanswered: ringing stops after 45 s and the call ends.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  c2 := public.call_start(p_friend => b);
  execute 'reset role';
  update call_members set invited_at = now() - interval '60 seconds' where call_id = c2 and member = b;
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  if jsonb_array_length(public.call_poll(0)->'calls') <> 0 then raise exception 'FAIL: missed call still live'; end if;
  execute 'reset role';
  if (select state from call_members where call_id = c2 and member = b) <> 'missed' then
    raise exception 'FAIL: unanswered call not missed';
  end if;

  -- ---------------------------------------------------------------- group call
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  gid := public.create_group('Sesli', array[b, c]);
  c3 := public.call_start(p_group => gid);
  execute 'reset role';
  if (select count(*) from call_members where call_id = c3 and state = 'ringing') <> 2 then
    raise exception 'FAIL: group members not ringing';
  end if;

  -- Deniz is not in the group.
  perform set_config('request.jwt.claims',
    json_build_object('sub', d, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.call_join(c3);
    raise exception 'FAIL: outsider joined a group call';
  exception when others then
    if sqlerrm <> 'friends.groupNotFound' then raise exception 'FAIL: outsider group join: %', sqlerrm; end if;
  end;
  begin
    perform public.call_start(p_group => gid);
    raise exception 'FAIL: outsider started a group call';
  exception when others then
    if sqlerrm <> 'friends.groupNotFound' then raise exception 'FAIL: outsider group start: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- Bob "starts" the group call: he joins the live one.
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  if public.call_start(p_group => gid) <> c3 then raise exception 'FAIL: second live call in a group'; end if;
  execute 'reset role';

  -- Cem lets it ring out, then joins later anyway.
  update call_members set invited_at = now() - interval '60 seconds' where call_id = c3 and member = c;
  perform set_config('request.jwt.claims',
    json_build_object('sub', c, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  p := public.call_poll(0);
  if p->'calls'->0->>'myState' <> 'missed' then raise exception 'FAIL: group ring did not stop %', p; end if;
  perform public.call_join(c3);
  p := public.call_poll(0);
  if p->'calls'->0->>'myState' <> 'joined' or jsonb_array_length(p->'calls'->0->'members') <> 3 then
    raise exception 'FAIL: late join %', p;
  end if;
  execute 'reset role';
  if (select joined_at from call_members where call_id = c3 and member = c)
     <= (select joined_at from call_members where call_id = c3 and member = b) then
    raise exception 'FAIL: join order';
  end if;

  -- Alice's launcher stops polling: after 20 s she counts as gone.
  update call_members set seen_at = now() - interval '30 seconds' where call_id = c3 and member = a;
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  p := public.call_poll(0);
  if jsonb_array_length(p->'calls'->0->'members') <> 2 then raise exception 'FAIL: stale member kept %', p; end if;
  execute 'reset role';
  if (select state from call_members where call_id = c3 and member = a) <> 'left' then
    raise exception 'FAIL: stale member not left';
  end if;

  -- Alice comes back and removes Cem from the group: he drops out of the call.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.call_join(c3);
  perform public.remove_group_member(gid, c);
  perform public.call_poll(0);
  execute 'reset role';
  if (select state from call_members where call_id = c3 and member = c) <> 'left' then
    raise exception 'FAIL: removed member still in the call';
  end if;

  -- Starting another call leaves this one.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  c2 := public.call_start(p_friend => c);
  execute 'reset role';
  if (select state from call_members where call_id = c3 and member = a) <> 'left' then
    raise exception 'FAIL: two calls at once';
  end if;
  if (select ended_at from calls where id = c3) is not null then
    raise exception 'FAIL: group call ended while Bob is in it';
  end if;

  -- The last member leaving ends the group call.
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.call_leave(c3);
  execute 'reset role';
  if (select ended_at from calls where id = c3) is null then raise exception 'FAIL: empty group call kept'; end if;

  -- ---------------------------------------------------------------- limits
  -- Banned: polls see nothing and cannot call.
  insert into bans (user_id, until, reason) values (c, null, 'test');
  perform set_config('request.jwt.claims',
    json_build_object('sub', c, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  if jsonb_array_length(public.call_poll(0)->'calls') <> 0 then raise exception 'FAIL: banned user sees calls'; end if;
  begin
    perform public.call_join(c2);
    raise exception 'FAIL: banned user joined';
  exception when others then
    if sqlerrm <> 'friends.disabled' then raise exception 'FAIL: banned join: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- Anonymous identities cannot call.
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', true)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.call_start(p_friend => a);
    raise exception 'FAIL: anonymous call';
  exception when others then
    if sqlerrm <> 'friends.disabled' then raise exception 'FAIL: anonymous call: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- At most 10 new calls a minute.
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  n := 0;
  begin
    for i in 1..12 loop
      perform public.call_leave(public.call_start(p_friend => b));
      n := n + 1;
    end loop;
    raise exception 'FAIL: no call rate limit';
  exception when others then
    if sqlerrm <> 'friends.rateLimited' then raise exception 'FAIL: call rate limit: %', sqlerrm; end if;
  end;
  if n > 10 then raise exception 'FAIL: % calls in a minute', n; end if;
  execute 'reset role';

  -- Internals are not callable.
  begin
    execute 'set local role authenticated';
    perform public.call_end(c2);
    raise exception 'FAIL: call_end is callable';
  exception when insufficient_privilege then null;
  end;
  execute 'reset role';

  raise exception 'PHASE24_OK all checks passed (nothing was saved)';
end;
$test$;
