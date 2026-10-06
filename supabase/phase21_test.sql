-- MehburMC Launcher — checks for phase21.sql. Run in Supabase → SQL Editor
-- after phase21.sql and phase21_fix1.sql. It always ends with an error so that nothing is saved:
--   "PHASE21_OK ..."   → all checks passed
--   "FAIL: ..."        → a rule is broken (send the message to the developer)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 5);
  founder uuid := gen_random_uuid();
  modr uuid := gen_random_uuid();
  alice uuid := gen_random_uuid();
  bob uuid := gen_random_uuid();
  anon_u uuid := gen_random_uuid();
  sha text := md5('p21' || random()::text) || '00000000';
  sha2 text := md5('p21b' || random()::text) || '00000000';
  share bigint;
  tex bigint;
  n int;
  ok boolean;
begin
  -- ---------------------------------------------------------------- setup (as postgres)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (founder, 'p21f' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (modr,    'p21m' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (alice,   'p21a' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (bob,     'p21b' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (anon_u,  null,                             'authenticated', 'authenticated', true,  now(), now());
  insert into account_names (name_lower, name, owner) values
    ('p21a_' || sfx, 'P21a_' || sfx, alice),
    ('p21b_' || sfx, 'P21b_' || sfx, bob);
  -- The test founder becomes rank 1 only inside this transaction.
  insert into admins (user_id, rank) values (founder, 1), (modr, 2);

  -- ---------------------------------------------------------------- retired design
  select count(*) into n from shared_textures t
   where t.sha1 in (select r.sha1 from retired_textures r) and not t.hidden;
  if n <> 0 then raise exception 'FAIL: % shares of the old design still visible', n; end if;
  begin
    insert into shared_textures (owner, kind, name, author, sha1, visibility)
    values (alice, 'skin', 'Old', 'P21a_' || sfx, '15f36107b3e370aa9b39b7327f421b0fc371f0b0', 'public');
    raise exception 'FAIL: the old design was shared again';
  exception when others then
    if sqlerrm <> 'textures.notShareable' then raise exception 'FAIL: retired share: %', sqlerrm; end if;
  end;

  -- ---------------------------------------------------------------- rank 2 has no access
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.admin_add_private_texture('skin', 'classic', 'X', sha);
    raise exception 'FAIL: rank 2 added a private texture';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: rank2 add: %', sqlerrm; end if;
  end;
  if public.can_write_private_texture() then raise exception 'FAIL: rank 2 may upload'; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- founder adds and grants
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  if not public.can_write_private_texture() then raise exception 'FAIL: founder may not upload'; end if;
  tex := public.admin_add_private_texture('skin', 'slim', 'MehburMC', sha);
  if public.admin_add_private_texture('skin', 'classic', 'MehburMC 2', sha) <> tex then
    raise exception 'FAIL: same image added twice';
  end if;
  begin
    perform public.admin_add_private_texture('cape', 'classic', 'Old', '550a2572b75449fd44f33723a4bea217ba767805');
    raise exception 'FAIL: the old design became private';
  exception when others then
    if sqlerrm <> 'textures.notShareable' then raise exception 'FAIL: retired private: %', sqlerrm; end if;
  end;
  perform public.admin_grant_texture(tex, alice);
  perform public.admin_grant_texture(tex, alice);  -- twice is fine
  begin
    perform public.admin_grant_texture(tex, anon_u);
    raise exception 'FAIL: granted to an anonymous identity';
  exception when others then
    if sqlerrm <> 'admin.userNotFound' then raise exception 'FAIL: grant anon: %', sqlerrm; end if;
  end;
  select count(*) into n from public.admin_texture_grants(tex);
  if n <> 1 then raise exception 'FAIL: % grants (want 1)', n; end if;
  select x.grants into n from public.admin_private_textures() x where x.id = tex;
  if n <> 1 then raise exception 'FAIL: admin list grant count %', n; end if;
  if not public.can_read_private_texture(sha || '.png') then
    raise exception 'FAIL: founder cannot read the file';
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- receivers
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.my_private_textures() x where x.id = tex and x.model = 'classic';
  if n <> 1 then raise exception 'FAIL: grant not visible to alice'; end if;
  if not public.can_read_private_texture(sha || '.png') then
    raise exception 'FAIL: alice cannot read her texture';
  end if;
  if public.can_write_private_texture() then raise exception 'FAIL: alice may upload'; end if;
  begin
    perform public.admin_private_textures();
    raise exception 'FAIL: alice listed private textures';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: alice admin list: %', sqlerrm; end if;
  end;
  execute 'reset role';
  -- Even the receiver cannot share it with the community.
  begin
    insert into shared_textures (owner, kind, name, author, sha1, visibility)
    values (alice, 'skin', 'Mine now', 'P21a_' || sfx, sha, 'public');
    raise exception 'FAIL: a private texture was shared';
  exception when others then
    if sqlerrm <> 'textures.notShareable' then raise exception 'FAIL: private share: %', sqlerrm; end if;
  end;

  perform set_config('request.jwt.claims',
    json_build_object('sub', bob, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.my_private_textures();
  if n <> 0 then raise exception 'FAIL: bob sees textures he was not given'; end if;
  if public.can_read_private_texture(sha || '.png') then
    raise exception 'FAIL: bob can read a texture he was not given';
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- banned receivers lose it
  insert into bans (user_id, until, reason) values (alice, null, 'p21');
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.my_private_textures();
  if n <> 0 then raise exception 'FAIL: banned user still has textures'; end if;
  execute 'reset role';
  delete from bans where user_id = alice;

  -- ---------------------------------------------------------------- revoke and delete
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  perform public.admin_revoke_texture(tex, alice);
  select count(*) into n from public.admin_texture_grants(tex);
  if n <> 0 then raise exception 'FAIL: revoke kept the grant'; end if;
  perform public.admin_grant_texture(tex, bob);
  if public.admin_delete_private_texture(tex) <> sha then
    raise exception 'FAIL: delete returned the wrong sha';
  end if;
  execute 'reset role';
  select count(*) into n from texture_grants g where g.texture_id = tex;
  if n <> 0 then raise exception 'FAIL: grants survived the delete'; end if;

  -- ---------------------------------------------------------------- community moderation (fix 1)
  insert into shared_textures (owner, kind, name, author, sha1, visibility)
  values (alice, 'skin', 'Friends only', 'P21a_' || sfx, sha2, 'friends')
  returning id into share;
  perform set_config('request.jwt.claims',
    json_build_object('sub', modr, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.admin_textures(false);
    raise exception 'FAIL: rank 2 listed all shares';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: rank2 list: %', sqlerrm; end if;
  end;
  if public.can_read_texture_file(alice::text || '/' || sha2 || '.png') then
    raise exception 'FAIL: rank 2 reads a friends-only image';
  end if;
  execute 'reset role';
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.admin_textures(false) a where a.id = share;
  if n <> 1 then raise exception 'FAIL: founder does not see a friends-only share'; end if;
  if not public.can_read_texture_file(alice::text || '/' || sha2 || '.png') then
    raise exception 'FAIL: founder cannot read the image';
  end if;
  perform public.admin_hide_texture(share);
  select count(*) into n from public.admin_textures(false) a where a.id = share;
  if n <> 0 then raise exception 'FAIL: removed share still listed'; end if;
  select count(*) into n from public.admin_textures(true) a where a.id = share;
  if n <> 1 then raise exception 'FAIL: removed share missing from the removed list'; end if;
  perform public.admin_restore_texture(share);
  select count(*) into n from public.admin_textures(false) a where a.id = share;
  if n <> 1 then raise exception 'FAIL: restore did not bring the share back'; end if;
  begin
    perform public.admin_restore_texture(-1);
    raise exception 'FAIL: restored a missing share';
  exception when others then
    if sqlerrm <> 'textures.notFound' then raise exception 'FAIL: restore missing: %', sqlerrm; end if;
  end;
  execute 'reset role';

  raise exception 'PHASE21_OK all checks passed (nothing was saved)';
end;
$test$;
