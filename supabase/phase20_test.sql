-- MehburMC Launcher — checks for phase20.sql. Run in Supabase → SQL Editor
-- after phase20.sql. It creates throwaway users, acts as each of them and
-- checks every admin/ban rule. It always ends with an error so that nothing
-- is saved:
--   "PHASE20_OK ..."   → all checks passed
--   "FAIL: ..."        → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 5);
  founder uuid := gen_random_uuid();
  modr uuid := gen_random_uuid();
  alice uuid := gen_random_uuid();
  bob uuid := gen_random_uuid();
  dave uuid := gen_random_uuid();
  anon_u uuid := gen_random_uuid();
  lib_id bigint;
  tex_id bigint;
  sha text := md5('p20' || random()::text) || '00000000';
  r record;
  n int;
  ok boolean;

begin
  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (founder, 'p20f' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (modr,    'p20m' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (alice,   'p20a' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (bob,     'p20b' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (dave,    'p20d' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (anon_u,  null,                             'authenticated', 'authenticated', true,  now(), now());
  insert into account_names (name_lower, name, owner) values
    ('p20f_' || sfx, 'P20f_' || sfx, founder),
    ('p20m_' || sfx, 'P20m_' || sfx, modr),
    ('p20a_' || sfx, 'P20a_' || sfx, alice),
    ('p20b_' || sfx, 'P20b_' || sfx, bob),
    ('p20d_' || sfx, 'P20d_' || sfx, dave),
    ('p20n_' || sfx, 'P20n_' || sfx, anon_u);
  insert into admins (user_id, rank) values (founder, 1), (anon_u, 2);
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan)
  values (alice, 'P20a_' || sfx, 'P20 Mod', 'p20mod', '1.0', array['fabric'], sha, 100, 'p20.jar',
          '{"verdict":"pass","findings":[]}')
  returning id into lib_id;
  insert into shared_textures (owner, kind, name, author, sha1, visibility)
  values (alice, 'skin', 'P20 Skin', 'P20a_' || sfx, sha, 'public')
  returning id into tex_id;
  insert into texture_reports (texture_id, reporter, reason) values (tex_id, dave, 'spam');

  -- ---------------------------------------------------------------- plain member
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_account();
  if r.rank <> 0 or r.banned then raise exception 'FAIL: member my_account %', r; end if;
  begin
    perform public.admin_library('approved');
    raise exception 'FAIL: member read the admin library';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: member admin_library: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_ban(bob, null, 'x');
    raise exception 'FAIL: member banned someone';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: member admin_ban: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- anonymous admin row is powerless
  perform set_config('request.jwt.claims',
    json_build_object('sub', anon_u, 'role', 'authenticated', 'is_anonymous', true)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.admin_bans();
    raise exception 'FAIL: anonymous identity used admin rights';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: anon admin_bans: %', sqlerrm; end if;
  end;
  execute 'reset role';
  delete from admins where user_id = anon_u;

  -- ---------------------------------------------------------------- founder appoints rank 2
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_account();
  if r.rank <> 1 then raise exception 'FAIL: founder rank %', r.rank; end if;
  perform public.admin_set_rank(modr, 2);
  begin
    perform public.admin_set_rank(anon_u, 2);
    raise exception 'FAIL: anonymous identity became admin';
  exception when others then
    if sqlerrm <> 'admin.userNotFound' then raise exception 'FAIL: rank anon: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_ban(founder, null, 'x');
    raise exception 'FAIL: founder banned themselves';
  exception when others then
    if sqlerrm <> 'admin.self' then raise exception 'FAIL: self ban: %', sqlerrm; end if;
  end;
  select count(*) into n from public.admin_list() l where l.user_id in (founder, modr);
  if n <> 2 then raise exception 'FAIL: admin_list has % of 2', n; end if;
  select count(*) into n from public.admin_find_users('p20');
  if n < 5 then raise exception 'FAIL: find_users found % (want >= 5)', n; end if;
  select count(*) into n from public.admin_find_users('p20n_' || sfx);
  if n <> 0 then raise exception 'FAIL: find_users lists anonymous identities'; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- rank 2 limits and bans
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_account();
  if r.rank <> 2 then raise exception 'FAIL: rank 2 my_account %', r.rank; end if;
  begin
    perform public.admin_set_rank(bob, 2);
    raise exception 'FAIL: rank 2 appointed an admin';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: rank2 set_rank: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_library('pending');
    raise exception 'FAIL: rank 2 saw the approval queue';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: rank2 pending: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_ban(founder, null, 'x');
    raise exception 'FAIL: rank 2 banned rank 1';
  exception when others then
    if sqlerrm <> 'admin.targetAdmin' then raise exception 'FAIL: ban admin: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_ban(bob, 0, 'x');
    raise exception 'FAIL: ban in the past accepted';
  exception when others then
    if sqlerrm <> 'admin.badUntil' then raise exception 'FAIL: past ban: %', sqlerrm; end if;
  end;
  perform public.admin_ban(bob, 24, 'spam');
  select count(*) into n from public.admin_bans() b where b.user_id = bob and b.active;
  if n <> 1 then raise exception 'FAIL: ban not listed'; end if;
  -- Rank 2 sees reported mods but no skins/capes.
  select count(*) into n from public.admin_reports() x where x.kind = 'texture' and x.item_id = tex_id;
  if n <> 0 then raise exception 'FAIL: rank 2 sees texture reports'; end if;
  begin
    perform public.admin_hide_texture(tex_id);
    raise exception 'FAIL: rank 2 hid a texture';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: rank2 hide: %', sqlerrm; end if;
  end;
  -- Admins may read pending files (to review them).
  if not public.can_read_library_file(alice::text || '/' || sha || '.jar') then
    raise exception 'FAIL: admin cannot read a pending file';
  end if;
  execute 'reset role';
  select (u.banned_until > now()) into ok from auth.users u where u.id = bob;
  if not coalesce(ok, false) then raise exception 'FAIL: auth.users.banned_until not set'; end if;

  -- ---------------------------------------------------------------- banned user
  perform set_config('request.jwt.claims',
    json_build_object('sub', bob, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.my_account();
  if not r.banned or r.ban_reason <> 'spam' or r.ban_until is null then
    raise exception 'FAIL: banned my_account %', r;
  end if;
  begin
    perform public.delete_me();
    raise exception 'FAIL: banned user deleted the account';
  exception when others then
    if sqlerrm <> 'auth.banned' then raise exception 'FAIL: banned delete_me: %', sqlerrm; end if;
  end;
  if public.can_read_library_file(alice::text || '/' || sha || '.jar') then
    raise exception 'FAIL: non-admin read a pending file';
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- founder reviews
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.admin_library('pending') x where x.id = lib_id;
  if n <> 1 then raise exception 'FAIL: pending mod not in the queue'; end if;
  begin
    perform public.admin_review_library_mod(lib_id, true, '',
      json_build_object('verdict', 'pass', 'sha1', repeat('0', 40))::jsonb);
    raise exception 'FAIL: approved without a matching scan';
  exception when others then
    if sqlerrm <> 'library.blocked' then raise exception 'FAIL: approve wrong sha: %', sqlerrm; end if;
  end;
  begin
    perform public.admin_review_library_mod(lib_id, true, '',
      json_build_object('verdict', 'block', 'sha1', sha)::jsonb);
    raise exception 'FAIL: approved a blocked scan';
  exception when others then
    if sqlerrm <> 'library.blocked' then raise exception 'FAIL: approve blocked: %', sqlerrm; end if;
  end;
  perform public.admin_review_library_mod(lib_id, true, 'ok',
    json_build_object('verdict', 'pass', 'sha1', sha)::jsonb);
  select count(*) into n from public.admin_reports() x where x.kind = 'texture' and x.item_id = tex_id;
  if n <> 1 then raise exception 'FAIL: rank 1 does not see texture reports'; end if;
  perform public.admin_hide_texture(tex_id);
  select count(*) into n from public.admin_reports() x where x.kind = 'texture' and x.item_id = tex_id;
  if n <> 0 then raise exception 'FAIL: hidden texture still reported'; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- users see approved mod, report it
  perform set_config('request.jwt.claims',
    json_build_object('sub', dave, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.library_list(300) x where x.id = lib_id;
  if n <> 1 then raise exception 'FAIL: approved mod not listed for users'; end if;
  perform public.report_library_mod(lib_id, 'malware', 'p20 test');
  execute 'reset role';

  -- ---------------------------------------------------------------- rank 2 takes it down
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.admin_reports() x where x.kind = 'mod' and x.item_id = lib_id and x.reports = 1;
  if n <> 1 then raise exception 'FAIL: reported mod not in admin_reports'; end if;
  perform public.admin_remove_library_mod(lib_id, 'malware');
  select count(*) into n from public.admin_reports() x where x.kind = 'mod' and x.item_id = lib_id;
  if n <> 0 then raise exception 'FAIL: removed mod still reported'; end if;
  perform public.admin_dismiss_reports('mod', lib_id);
  perform public.admin_unban(bob);
  execute 'reset role';
  select count(*) into n from library_mods m where m.id = lib_id and m.status = 'rejected';
  if n <> 1 then raise exception 'FAIL: removed mod not rejected'; end if;
  select count(*) into n from auth.users u where u.id = bob and u.banned_until is null;
  if n <> 1 then raise exception 'FAIL: unban did not clear banned_until'; end if;

  -- ---------------------------------------------------------------- founder removes rank 2
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.admin_set_rank(modr, 0);
  execute 'reset role';
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.admin_bans();
    raise exception 'FAIL: removed admin kept rights';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: removed admin: %', sqlerrm; end if;
  end;
  execute 'reset role';

  raise exception 'PHASE20_OK all checks passed (nothing was saved)';
end;
$test$;
