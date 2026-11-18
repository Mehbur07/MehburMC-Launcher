-- MehburMC Launcher — site_account.sql için denetim. Supabase → SQL Editor'de
-- site_account.sql'den sonra çalıştır. Geçici kullanıcılar oluşturur, her birinin
-- yerine geçer ve hiçbir şey kaydedilmesin diye her zaman hatayla biter:
--   "SITE_ACCOUNT_OK ..."  → bütün denetimler geçti
--   "FAIL: ..."            → bir kural bozuk (mesajı geliştiriciye gönder)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 6);
  alice uuid := gen_random_uuid();
  bob uuid := gen_random_uuid();
  names text[];
  n int;
begin
  if to_regprocedure('public.my_account_names()') is null then
    raise exception 'FAIL: run site_account.sql first (my_account_names is missing)';
  end if;

  -- ---------------------------------------------------------------- hazırlık (postgres olarak)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (alice, 'saa' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (bob,   'sab' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now());
  insert into account_names (name_lower, name, owner, created_at) values
    (lower('Sa_' || sfx), 'Sa_' || sfx, alice, now() - interval '2 days'),
    (lower('Sb_' || sfx), 'Sb_' || sfx, alice, now() - interval '1 day'),
    (lower('Sc_' || sfx), 'Sc_' || sfx, bob, now());

  -- ---------------------------------------------------------------- Alice yalnızca kendi adlarını görür
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select array_agg(x.name) into names from public.my_account_names() x;
  if names is distinct from array['Sa_' || sfx, 'Sb_' || sfx] then
    raise exception 'FAIL: alice sees %', names;
  end if;
  -- Site ad eklerken launcher'ın claim_name'ini kullanır; liste hemen güncellenir.
  perform public.claim_name('Sd_' || sfx);
  select count(*) into n from public.my_account_names();
  if n <> 3 then raise exception 'FAIL: claimed name not listed (% names)', n; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- Bob, Alice'in adlarını görmez
  perform set_config('request.jwt.claims',
    json_build_object('sub', bob, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select array_agg(x.name) into names from public.my_account_names() x;
  if names is distinct from array['Sc_' || sfx] then
    raise exception 'FAIL: bob sees %', names;
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- ziyaretçi çağıramaz
  perform set_config('request.jwt.claims', json_build_object('role', 'anon')::text, true);
  execute 'set local role anon';
  begin
    perform 1 from public.my_account_names();
    raise exception 'FAIL: visitor called my_account_names';
  exception when insufficient_privilege then null;
  end;
  execute 'reset role';

  raise exception 'SITE_ACCOUNT_OK all checks passed (nothing was saved)';
end;
$test$;
