-- MehburMC Launcher — checks for phase23.sql. Run in Supabase → SQL Editor
-- after phase23.sql. It creates throwaway users, acts as each of them and
-- always ends with an error so that nothing is saved:
--   "PHASE23_OK ..."   → all checks passed
--   "FAIL: ..."        → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 4);
  a uuid := gen_random_uuid();
  b uuid := gen_random_uuid();
  c uuid := gen_random_uuid();
  d uuid := gen_random_uuid();
  many uuid[] := '{}';
  u uuid;
  gid bigint;
  g2 bigint;
  r record;
  n int;
  err text;
begin
  if to_regprocedure('public.create_group(text, uuid[])') is null then
    raise exception 'FAIL: run phase23.sql first (create_group is missing)';
  end if;

  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at)
  select x, 'p23' || left(x::text, 8) || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()
    from unnest(array[a, b, c, d]) x;
  insert into profiles (id, friend_code, display_name) values
    (a, 'P23A-' || upper(sfx), 'Alice'), (b, 'P23B-' || upper(sfx), 'Bob'),
    (c, 'P23C-' || upper(sfx), 'Cem'), (d, 'P23D-' || upper(sfx), 'Deniz');
  insert into friendships (requester, addressee, status) values
    (a, b, 'accepted'), (a, c, 'accepted'), (b, d, 'accepted');
  -- 15 more friends of Alice for the size limit.
  for i in 1..15 loop
    u := gen_random_uuid();
    insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at)
    values (u, 'p23m' || i || left(u::text, 6) || '@test.invalid', 'authenticated', 'authenticated', false, now(), now());
    insert into profiles (id, friend_code, display_name) values (u, 'P23M' || i || '-' || upper(sfx), 'M' || i);
    insert into friendships (requester, addressee, status) values (a, u, 'accepted');
    many := many || u;
  end loop;

  -- ---------------------------------------------------------------- nicknames (as Alice)
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.set_nickname(b, '  Bobby   the  Builder ');
  select f.nickname into r from public.my_friends() f where f.id = b;
  if r.nickname is distinct from 'Bobby the Builder' then raise exception 'FAIL: nickname %', r.nickname; end if;
  begin
    perform public.set_nickname(d, 'Stranger');
    raise exception 'FAIL: nickname for a non-friend';
  exception when others then
    if sqlerrm <> 'friends.notFriends' then raise exception 'FAIL: non-friend nickname: %', sqlerrm; end if;
  end;
  select count(*) into n from friend_nicknames;
  if n <> 0 then raise exception 'FAIL: nickname table readable'; end if;
  select count(*) into n from chat_groups;
  if n <> 0 then raise exception 'FAIL: group table readable'; end if;
  execute 'reset role';

  -- Bob does not see Alice's name for him.
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select f.nickname into r from public.my_friends() f where f.id = a;
  if r.nickname is not null then raise exception 'FAIL: friend sees my nickname'; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- groups (as Alice)
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.create_group('Bad', array[d]);
    raise exception 'FAIL: group with a non-friend';
  exception when others then
    if sqlerrm <> 'friends.notFriends' then raise exception 'FAIL: non-friend member: %', sqlerrm; end if;
  end;
  begin
    perform public.create_group('   ', array[b]);
    raise exception 'FAIL: empty group name';
  exception when others then
    if sqlerrm <> 'friends.groupNameInvalid' then raise exception 'FAIL: empty name: %', sqlerrm; end if;
  end;
  begin
    perform public.create_group('Huge', many || b);
    raise exception 'FAIL: 17 members allowed';
  exception when others then
    if sqlerrm <> 'friends.groupFull' then raise exception 'FAIL: size limit: %', sqlerrm; end if;
  end;
  gid := public.create_group('Squad', array[b, c]);
  select * into r from public.my_groups() g where g.id = gid;
  if r.owner <> a or jsonb_array_length(r.members) <> 3 or r.unread <> 0 then
    raise exception 'FAIL: new group %', r;
  end if;
  if not exists (select 1 from jsonb_array_elements(r.members) m
                  where m->>'id' = b::text and m->>'nickname' = 'Bobby the Builder') then
    raise exception 'FAIL: own nickname missing in members %', r.members;
  end if;
  begin
    perform public.add_group_members(gid, array[d]);
    raise exception 'FAIL: added a non-friend';
  exception when others then
    if sqlerrm <> 'friends.notFriends' then raise exception 'FAIL: add non-friend: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- Bob: messages, no owner rights
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  insert into group_messages (group_id, sender, body) values (gid, b, 'selam');
  select * into r from public.my_groups() g where g.id = gid;
  if r.unread <> 0 then raise exception 'FAIL: own message unread'; end if;
  if exists (select 1 from jsonb_array_elements(r.members) m where m->>'nickname' is not null) then
    raise exception 'FAIL: Bob sees nicknames %', r.members;
  end if;
  begin
    insert into group_messages (group_id, sender, body) values (gid, a, 'spoof');
    raise exception 'FAIL: sent as someone else';
  exception when insufficient_privilege then null;
  end;
  begin
    perform public.rename_group(gid, 'Mine');
    raise exception 'FAIL: member renamed';
  exception when others then
    if sqlerrm <> 'friends.notOwner' then raise exception 'FAIL: member rename: %', sqlerrm; end if;
  end;
  begin
    perform public.add_group_members(gid, array[d]);
    raise exception 'FAIL: member added people';
  exception when others then
    if sqlerrm <> 'friends.notOwner' then raise exception 'FAIL: member add: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- Deniz: outsider
  perform set_config('request.jwt.claims',
    json_build_object('sub', d, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from group_messages m where m.group_id = gid;
  if n <> 0 then raise exception 'FAIL: outsider reads the group'; end if;
  begin
    insert into group_messages (group_id, sender, body) values (gid, d, 'hi');
    raise exception 'FAIL: outsider posted';
  exception when insufficient_privilege then null;
  end;
  begin
    perform public.leave_group(gid);
    raise exception 'FAIL: outsider left';
  exception when others then
    if sqlerrm <> 'friends.groupNotFound' then raise exception 'FAIL: outsider leave: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- Alice: unread, remove, leave
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_groups() g where g.id = gid;
  if r.unread <> 1 then raise exception 'FAIL: unread %', r.unread; end if;
  perform public.mark_group_read(gid);
  select * into r from public.my_groups() g where g.id = gid;
  if r.unread <> 0 then raise exception 'FAIL: still unread'; end if;
  perform public.rename_group(gid, '  Yeni   Ekip ');
  perform public.remove_group_member(gid, c);
  select * into r from public.my_groups() g where g.id = gid;
  if r.name <> 'Yeni Ekip' or jsonb_array_length(r.members) <> 2 then
    raise exception 'FAIL: rename/remove %', r;
  end if;
  perform public.leave_group(gid);
  if exists (select 1 from public.my_groups() g where g.id = gid) then
    raise exception 'FAIL: still in the group after leaving';
  end if;
  execute 'reset role';

  -- Bob inherited it; leaving last deletes it.
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_groups() g where g.id = gid;
  if r.owner <> b then raise exception 'FAIL: owner not handed over: %', r.owner; end if;
  perform public.rename_group(gid, 'Bob');
  perform public.leave_group(gid);
  execute 'reset role';
  if exists (select 1 from chat_groups g where g.id = gid) then
    raise exception 'FAIL: empty group kept';
  end if;

  -- ---------------------------------------------------------------- owner deletes friend data
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  g2 := public.create_group('Kalici', array[b, c]);
  perform public.delete_friend_data();
  execute 'reset role';
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_groups() g where g.id = g2;
  if r.id is null or r.owner not in (b, c) or jsonb_array_length(r.members) <> 2 then
    raise exception 'FAIL: group lost with its owner %', r;
  end if;
  if r.owner = b then perform public.rename_group(g2, 'Hala burada'); end if;
  execute 'reset role';

  raise exception 'PHASE23_OK all checks passed (nothing was saved)';
end;
$test$;
