-- MehburMC Launcher — checks for library_packs.sql. Run in Supabase → SQL
-- Editor after library_packs.sql. It creates throwaway users, acts as each
-- of them and always ends with an error so that nothing is saved:
--   "LIBRARY_PACKS_OK ..." → all checks passed
--   "FAIL: ..."            → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 6);
  alice uuid := gen_random_uuid();
  bob uuid := gen_random_uuid();
  founder uuid := gen_random_uuid();
  author text;
  sha_mod text := md5('m' || random()::text) || '00000000';
  sha_rp text := md5('r' || random()::text) || '00000000';
  sha_mp text := md5('p' || random()::text) || '00000000';
  sha_big text := md5('b' || random()::text) || '00000000';
  rp bigint;
  mp bigint;
  n int;
  ok boolean;
  scan_rp jsonb;
begin
  if to_regprocedure('public.library_ext(text)') is null then
    raise exception 'FAIL: run library_packs.sql first (library_ext is missing)';
  end if;
  author := 'Lp_' || sfx;
  scan_rp := jsonb_build_object('verdict', 'pass', 'kind', 'resourcepack', 'sha1', sha_rp, 'findings', '[]'::jsonb);

  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (alice,   'lpa' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (bob,     'lpb' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (founder, 'lpf' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now());
  insert into account_names (name_lower, name, owner) values (lower(author), author, alice);
  insert into admins (user_id, rank) values (founder, 1);

  -- ---------------------------------------------------------------- table rules
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind, status)
  values (alice, author, 'Old Mod', 'oldmod', '1.0', array['fabric'], sha_mod, 100, 'old.jar',
          '{"verdict":"pass"}', 'mod', 'approved');
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind, status)
  values (alice, author, 'Faithful', 'faithful', '1', '{}', sha_rp, 40000000, 'faithful.zip',
          scan_rp, 'resourcepack', 'approved')
  returning id into rp;
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
  values (alice, author, 'Pack', 'pack', '2.0', array['fabric'], sha_mp, 1000, 'pack.mrpack',
          '{"verdict":"warn","kind":"modpack"}', 'modpack')
  returning id into mp;

  begin
    insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
    values (alice, author, 'Bad', 'bad', '1', '{}', md5('x') || '00000000', 10, 'bad.jar', '{}', 'mod');
    raise exception 'FAIL: mod without a loader';
  exception when check_violation then null;
  end;
  begin
    insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
    values (alice, author, 'Bad', 'bad', '1', '{}', md5('y') || '00000000', 10, 'bad.jar', '{}', 'shader');
    raise exception 'FAIL: shader with a .jar name';
  exception when check_violation then null;
  end;
  begin
    insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
    values (alice, author, 'Big', 'big', '1', array['fabric'], md5('z') || '00000000', 30000000, 'big.jar', '{}', 'mod');
    raise exception 'FAIL: 30 MB mod';
  exception when check_violation then null;
  end;
  begin
    insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
    values (alice, author, 'Huge', 'huge', '1', '{}', md5('w') || '00000000', 60000000, 'huge.zip', '{}', 'resourcepack');
    raise exception 'FAIL: 60 MB pack';
  exception when check_violation then null;
  end;
  begin
    insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name, scan, kind)
    values (alice, author, 'X', 'x', '1', '{}', md5('v') || '00000000', 10, 'x.zip', '{}', 'datapack');
    raise exception 'FAIL: unknown kind';
  exception when check_violation then null;
  end;

  -- ---------------------------------------------------------------- lists (as Bob)
  perform set_config('request.jwt.claims',
    json_build_object('sub', bob, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  -- Old launchers (no p_all): mods only.
  select count(*) into n from public.library_list(300) l where l.owner = alice;
  if n <> 1 then raise exception 'FAIL: old list shows % items, expected the mod only', n; end if;
  select count(*) into n from public.library_list(300, true) l where l.owner = alice;
  if n <> 2 then raise exception 'FAIL: new list shows % items, expected mod + approved pack', n; end if;
  select l.kind = 'resourcepack' into ok from public.library_list(300, true) l where l.id = rp;
  if not ok then raise exception 'FAIL: kind not listed'; end if;
  -- Storage paths carry the kind's extension.
  if not public.can_read_library_file(alice::text || '/' || sha_rp || '.zip') then
    raise exception 'FAIL: approved pack file not readable';
  end if;
  if public.can_read_library_file(alice::text || '/' || sha_rp || '.jar') then
    raise exception 'FAIL: wrong extension readable';
  end if;
  if public.can_read_library_file(alice::text || '/' || sha_mp || '.mrpack') then
    raise exception 'FAIL: pending modpack readable by others';
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- submit (as Alice)
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.library_list(300, true) l where l.mine;
  if n <> 3 then raise exception 'FAIL: own uploads % (expected 3)', n; end if;
  begin
    perform public.submit_library_mod('S', '', author, 'shader', '1', '{}', '', md5('s') || '00000000',
      100, 's.zip', '{"verdict":"pass","kind":"shader"}'::jsonb, 'datapack');
    raise exception 'FAIL: unknown kind submitted';
  exception when others then
    if sqlerrm <> 'library.badKind' then raise exception 'FAIL: bad kind: %', sqlerrm; end if;
  end;
  begin
    -- A mod report cannot back a shader upload.
    perform public.submit_library_mod('S', '', author, 'shader', '1', '{}', '', md5('s') || '00000000',
      100, 's.zip', '{"verdict":"pass"}'::jsonb, 'shader');
    raise exception 'FAIL: report of another kind accepted';
  exception when others then
    if sqlerrm <> 'library.blocked' then raise exception 'FAIL: kind mismatch: %', sqlerrm; end if;
  end;
  begin
    perform public.submit_library_mod('S', '', author, 'shader', '1', '{}', '', md5('s') || '00000000',
      100, 's.zip', '{"verdict":"pass","kind":"shader"}'::jsonb, 'shader');
    raise exception 'FAIL: submitted without the file';
  exception when others then
    if sqlerrm <> 'library.fileMissing' then raise exception 'FAIL: valid shader: %', sqlerrm; end if;
  end;
  begin
    -- 40 MB + 1000 B + 100 B already; 170 MB more passes 200 MiB.
    perform public.submit_library_mod('Big', '', author, 'big', '1', '{}', '', sha_big,
      170000000, 'big.zip', '{"verdict":"pass","kind":"resourcepack"}'::jsonb, 'resourcepack');
    raise exception 'FAIL: storage quota not enforced';
  exception when others then
    if sqlerrm <> 'library.storageFull' then raise exception 'FAIL: storage quota: %', sqlerrm; end if;
  end;
  begin
    -- Old launchers call without p_kind: still a mod.
    perform public.submit_library_mod('M', '', author, 'm', '1', array['fabric'], '', md5('o') || '00000000',
      100, 'm.jar', '{"verdict":"pass"}'::jsonb);
    raise exception 'FAIL: old submit without the file';
  exception when others then
    if sqlerrm <> 'library.fileMissing' then raise exception 'FAIL: old submit: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- admin review
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.admin_library('pending') a where a.id = mp and a.kind = 'modpack';
  if n <> 1 then raise exception 'FAIL: admin does not see the pending modpack with its kind'; end if;
  if not public.can_read_library_file(alice::text || '/' || sha_mp || '.mrpack') then
    raise exception 'FAIL: admin cannot read the pending modpack';
  end if;
  begin
    perform public.admin_review_library_mod(mp, true, '',
      jsonb_build_object('sha1', sha_mp, 'verdict', 'pass', 'kind', 'mod'));
    raise exception 'FAIL: approved with a scan of another kind';
  exception when others then
    if sqlerrm <> 'library.blocked' then raise exception 'FAIL: review kind: %', sqlerrm; end if;
  end;
  perform public.admin_review_library_mod(mp, true, '',
    jsonb_build_object('sha1', sha_mp, 'verdict', 'warn', 'kind', 'modpack'));
  execute 'reset role';
  if (select status from library_mods where id = mp) <> 'approved' then
    raise exception 'FAIL: modpack not approved';
  end if;

  raise exception 'LIBRARY_PACKS_OK all checks passed (nothing was saved)';
end;
$test$;
