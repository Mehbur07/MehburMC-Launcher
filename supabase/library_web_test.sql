-- MehburMC Launcher — library_web.sql için denetim. Supabase → SQL Editor'de
-- library_web.sql'den sonra çalıştır. Geçici kullanıcılar oluşturur, her birinin
-- (ve giriş yapmamış ziyaretçinin) yerine geçer ve hiçbir şey kaydedilmesin diye
-- her zaman hatayla biter:
--   "LIBRARY_WEB_OK ..."   → bütün denetimler geçti
--   "FAIL: ..."            → bir kural bozuk (mesajı geliştiriciye gönder)

do $test$
declare
  sfx text := substr(md5(random()::text), 1, 6);
  alice uuid := gen_random_uuid();   -- içerik yükleyen oyuncu
  founder uuid := gen_random_uuid(); -- rank 1 admin
  author text;
  sha_mod text := md5('m' || random()::text) || '00000000';
  sha_sh text := md5('s' || random()::text) || '00000000';
  sha_pen text := md5('p' || random()::text) || '00000000';
  sha_rej text := md5('r' || random()::text) || '00000000';
  sha_new text := md5('n' || random()::text) || '00000000';
  id_mod bigint;
  id_sh bigint;
  id_pen bigint;
  id_rej bigint;
  id_new bigint;
  n int;
  r record;
  sig text;
  web_note text := '';
begin
  if to_regprocedure('public.library_public_list()') is null then
    raise exception 'FAIL: run library_web.sql first (library_public_list is missing)';
  end if;
  author := 'Lw_' || sfx;

  -- ---------------------------------------------------------------- hazırlık (postgres olarak)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at) values
    (alice,   'lwa' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()),
    (founder, 'lwf' || sfx || '@test.invalid', 'authenticated', 'authenticated', false, now(), now());
  insert into account_names (name_lower, name, owner) values (lower(author), author, alice);
  insert into admins (user_id, rank) values (founder, 1);

  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name,
                            scan, kind, status, review_note, reviewed_at)
  values (alice, author, 'Onaylı Mod', 'okmod', '1.0', array['fabric'], sha_mod, 100, 'ok.jar',
          '{"verdict":"pass","findings":[]}', 'mod', 'approved', 'gizli not', now())
  returning id into id_mod;
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name,
                            scan, kind, status, source)
  values (alice, author, 'Shader', 'shader', '1', '{}', sha_sh, 200, 'sh.zip',
          '{"verdict":"pass","kind":"shader"}', 'shader', 'approved', 'web')
  returning id into id_sh;
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name,
                            scan, kind, source)
  values (alice, author, 'Bekleyen', 'pending', '1', array['forge'], sha_pen, 300, 'p.jar',
          '{"verdict":"warn"}', 'mod', 'web')
  returning id into id_pen;
  insert into library_mods (owner, author, name, mod_id, version, loaders, sha1, size, file_name,
                            scan, kind, status)
  values (alice, author, 'Reddedilen', 'rejected', '1', '{}', sha_rej, 400, 'r.zip',
          '{"verdict":"pass","kind":"resourcepack"}', 'resourcepack', 'rejected')
  returning id into id_rej;

  -- Kaynak sütunu: varsayılan 'launcher', bilinmeyen değer reddedilir.
  if (select source from library_mods where id = id_mod) <> 'launcher' then
    raise exception 'FAIL: default source is not launcher';
  end if;
  begin
    update library_mods set source = 'cdn' where id = id_mod;
    raise exception 'FAIL: unknown source stored';
  exception when check_violation then null;
  end;

  -- Herkese açık liste gizli alanları döndürmez.
  sig := pg_get_function_result('public.library_public_list()'::regprocedure);
  if sig ~ '\m(owner|scan|review_note|review_scan|reviewed_by|status)\M' then
    raise exception 'FAIL: public list exposes a private column: %', sig;
  end if;

  -- Ziyaretçi için depo okuma kuralı.
  select count(*) into n from pg_policies p
   where p.schemaname = 'storage' and p.tablename = 'objects'
     and p.policyname = 'library_public_read' and p.cmd = 'SELECT' and p.roles = '{anon}';
  if n <> 1 then raise exception 'FAIL: storage policy for visitors is missing'; end if;

  -- ---------------------------------------------------------------- ziyaretçi (anon)
  perform set_config('request.jwt.claims', json_build_object('role', 'anon')::text, true);
  execute 'set local role anon';
  select count(*) into n from public.library_public_list() l
   where l.id in (id_mod, id_sh, id_pen, id_rej);
  if n <> 2 then raise exception 'FAIL: visitor sees % items, expected the 2 approved', n; end if;
  select * into r from public.library_public_list() l where l.id = id_sh;
  if r.file_path is distinct from alice::text || '/' || sha_sh || '.zip'
     or r.kind <> 'shader' or r.source <> 'web' or r.approved_at is null then
    raise exception 'FAIL: public row %', r;
  end if;
  select * into r from public.library_public_list() l where l.id = id_mod;
  if r.file_path is distinct from alice::text || '/' || sha_mod || '.jar' or r.source <> 'launcher' then
    raise exception 'FAIL: public mod row %', r;
  end if;
  -- En son onaylanan başta.
  select l.id into id_new from public.library_public_list() l
   where l.id in (id_mod, id_sh) limit 1;
  if id_new <> id_sh then raise exception 'FAIL: newest approval is not first'; end if;
  id_new := null;

  if not public.library_file_is_public(alice::text || '/' || sha_mod || '.jar') then
    raise exception 'FAIL: approved file not downloadable';
  end if;
  if public.library_file_is_public(alice::text || '/' || sha_mod || '.zip') then
    raise exception 'FAIL: wrong extension downloadable';
  end if;
  if public.library_file_is_public(alice::text || '/' || sha_pen || '.jar') then
    raise exception 'FAIL: pending file downloadable';
  end if;
  if public.library_file_is_public(alice::text || '/' || sha_rej || '.zip') then
    raise exception 'FAIL: rejected file downloadable';
  end if;

  begin
    perform 1 from public.library_list(300, true);
    raise exception 'FAIL: visitor called library_list';
  exception when insufficient_privilege then null;
  end;
  begin
    perform public.submit_library_mod('X', '', author, 'x', '1', array['fabric'], '', sha_new,
      100, 'x.jar', '{"verdict":"pass"}'::jsonb, 'mod', 'web');
    raise exception 'FAIL: visitor submitted';
  exception when insufficient_privilege then null;
  end;
  begin
    perform 1 from public.admin_library('pending');
    raise exception 'FAIL: visitor called admin_library';
  exception when insufficient_privilege then null;
  end;
  begin
    perform 1 from library_mods;
    raise exception 'FAIL: visitor read the table';
  exception when insufficient_privilege then null;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- yükleyen (Alice)
  perform set_config('request.jwt.claims',
    json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select count(*) into n from public.library_list(300, true) l where l.mine;
  if n <> 4 then raise exception 'FAIL: launcher list shows % own items (expected 4)', n; end if;
  select count(*) into n from public.library_public_list() l where l.id in (id_mod, id_sh);
  if n <> 2 then raise exception 'FAIL: signed-in user cannot use the public list'; end if;
  begin
    perform public.submit_library_mod('X', '', author, 'x', '1', array['fabric'], '', sha_new,
      100, 'x.jar', '{"verdict":"pass"}'::jsonb, 'mod', 'cdn');
    raise exception 'FAIL: unknown source submitted';
  exception when others then
    if sqlerrm <> 'library.badSource' then raise exception 'FAIL: bad source: %', sqlerrm; end if;
  end;
  begin
    -- Eski launcher'lar p_kind ve p_source göndermez: fonksiyon yine bulunur.
    perform public.submit_library_mod('X', '', author, 'x', '1', array['fabric'], '', sha_new,
      100, 'x.jar', '{"verdict":"pass"}'::jsonb);
    raise exception 'FAIL: old submit without the file';
  exception when others then
    if sqlerrm <> 'library.fileMissing' then raise exception 'FAIL: old submit: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- Dosya varken siteden yükleme 'web' olarak kaydedilir. Depo tablosuna
  -- yazılamıyorsa bu adım atlanır (sonuç mesajında yazar).
  begin
    insert into storage.objects (bucket_id, name, owner)
    values ('library', alice::text || '/' || sha_new || '.jar', alice);
  exception when others then
    web_note := ' (web submit skipped: ' || sqlerrm || ')';
  end;
  if web_note = '' then
    perform set_config('request.jwt.claims',
      json_build_object('sub', alice, 'role', 'authenticated', 'is_anonymous', false)::text, true);
    execute 'set local role authenticated';
    id_new := public.submit_library_mod('Yeni', '', author, 'newmod', '1', array['fabric'], '',
      sha_new, 100, 'new.jar', '{"verdict":"pass"}'::jsonb, 'mod', 'web');
    execute 'reset role';
    if (select source from library_mods where id = id_new) <> 'web' then
      raise exception 'FAIL: web upload not marked';
    end if;
    -- Aynı dosya launcher'dan yeniden gönderilirse kaynak güncellenir.
    execute 'set local role authenticated';
    perform public.submit_library_mod('Yeni', '', author, 'newmod', '1', array['fabric'], '',
      sha_new, 100, 'new.jar', '{"verdict":"pass"}'::jsonb);
    execute 'reset role';
    if (select source from library_mods where id = id_new) <> 'launcher' then
      raise exception 'FAIL: resubmit kept the old source';
    end if;
  end if;

  -- ---------------------------------------------------------------- admin
  perform set_config('request.jwt.claims',
    json_build_object('sub', founder, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.admin_library('pending') a where a.id = id_pen;
  if r.id is null or r.source <> 'web' or r.kind <> 'mod' then
    raise exception 'FAIL: admin list row %', r;
  end if;
  select * into r from public.admin_library('approved') a where a.id = id_mod;
  if r.source <> 'launcher' then raise exception 'FAIL: admin list source %', r.source; end if;
  execute 'reset role';

  raise exception 'LIBRARY_WEB_OK all checks passed (nothing was saved)%', web_note;
end;
$test$;
