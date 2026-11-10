-- MehburMC Launcher — support.sql için denetim. Supabase → SQL Editor'de
-- support.sql'den sonra çalıştır. Geçici kullanıcılar oluşturur, her birinin
-- yerine geçer ve hiçbir şey kaydedilmesin diye her zaman hatayla biter:
--   "SUPPORT_OK ..."   → bütün denetimler geçti
--   "FAIL: ..."        → bir kural bozuk (mesajı geliştiriciye gönder)

do $test$
declare
  a uuid := gen_random_uuid();    -- oyuncu
  b uuid := gen_random_uuid();    -- başka bir oyuncu
  adm uuid := gen_random_uuid();  -- rank 2 admin
  c uuid := gen_random_uuid();    -- banlı oyuncu
  tid bigint;
  t2 bigint;
  r record;
  n int;
begin
  if to_regprocedure('public.support_open(text, text)') is null then
    raise exception 'FAIL: run support.sql first (support_open is missing)';
  end if;

  -- ---------------------------------------------------------------- hazırlık (postgres olarak)
  insert into auth.users (id, email, aud, role, is_anonymous, created_at, updated_at)
  select x, 'sup' || left(x::text, 8) || '@test.invalid', 'authenticated', 'authenticated', false, now(), now()
    from unnest(array[a, b, adm, c]) x;
  insert into admins (user_id, rank) values (adm, 2);
  insert into bans (user_id, until, reason) values (c, null, 'test');

  -- ---------------------------------------------------------------- oyuncu konu açar
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  tid := public.support_open('  Ban   itirazı ', 'Merhaba, yardım eder misiniz?');
  select * into r from public.support_my_threads() x where x.id = tid;
  if r.subject is distinct from 'Ban itirazı' or r.status <> 'open' or r.unread <> 0 then
    raise exception 'FAIL: new thread %', r;
  end if;
  select x.mine into n from public.support_unread() x;
  if n <> 0 then raise exception 'FAIL: own message counted as unread %', n; end if;
  begin
    perform public.support_open('   ', 'boş konu');
    raise exception 'FAIL: empty subject accepted';
  exception when others then
    if sqlerrm <> 'support.subjectInvalid' then raise exception 'FAIL: empty subject: %', sqlerrm; end if;
  end;
  begin
    perform public.support_send(tid, repeat('x', 2001));
    raise exception 'FAIL: long message accepted';
  exception when others then
    if sqlerrm <> 'friends.messageInvalid' then raise exception 'FAIL: long message: %', sqlerrm; end if;
  end;
  begin
    perform 1 from support_threads;
    raise exception 'FAIL: table readable';
  exception when insufficient_privilege then null;
  end;
  begin
    perform public.admin_support_threads(true);
    raise exception 'FAIL: player listed admin threads';
  exception when others then
    if sqlerrm <> 'admin.forbidden' then raise exception 'FAIL: admin list: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- başka oyuncu göremez
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.support_thread_messages(tid, 0);
    raise exception 'FAIL: stranger read the thread';
  exception when others then
    if sqlerrm <> 'support.notFound' then raise exception 'FAIL: stranger read: %', sqlerrm; end if;
  end;
  begin
    perform public.support_send(tid, 'araya giriyorum');
    raise exception 'FAIL: stranger wrote to the thread';
  exception when others then
    if sqlerrm <> 'support.notFound' then raise exception 'FAIL: stranger write: %', sqlerrm; end if;
  end;
  begin
    perform public.support_set_open(tid, false);
    raise exception 'FAIL: stranger closed the thread';
  exception when others then
    if sqlerrm <> 'support.notFound' then raise exception 'FAIL: stranger close: %', sqlerrm; end if;
  end;
  if exists (select 1 from public.support_my_threads()) then
    raise exception 'FAIL: stranger sees threads';
  end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- admin cevaplar
  perform set_config('request.jwt.claims',
    json_build_object('sub', adm, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.admin_support_threads(true) x where x.id = tid;
  if r.id is null or r.unread <> 1 or r.owner <> a or r.preview <> 'Merhaba, yardım eder misiniz?'
     or r.last_from_admin then
    raise exception 'FAIL: admin list %', r;
  end if;
  select x.admin into n from public.support_unread() x;
  if n < 1 then raise exception 'FAIL: admin badge %', n; end if;
  perform public.support_send(tid, 'Tabii, anlat.');
  select * into r from public.admin_support_threads(true) x where x.id = tid;
  if r.unread <> 0 or not r.last_from_admin then raise exception 'FAIL: after reply %', r; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- oyuncu cevabı görür
  perform set_config('request.jwt.claims',
    json_build_object('sub', a, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select x.mine into n from public.support_unread() x;
  if n <> 1 then raise exception 'FAIL: player badge %', n; end if;
  select count(*) into n from public.support_thread_messages(tid, 0) m
   where (m.from_admin and m.author is null and not m.mine)
      or (not m.from_admin and m.mine);
  if n <> 2 then raise exception 'FAIL: player view of messages %', n; end if;
  perform public.support_mark_read(tid);
  select x.unread into n from public.support_my_threads() x where x.id = tid;
  if n <> 0 then raise exception 'FAIL: mark read %', n; end if;
  -- Kapatınca yazılamaz, oyuncu yeniden açamaz.
  perform public.support_set_open(tid, false);
  begin
    perform public.support_send(tid, 'bir şey daha');
    raise exception 'FAIL: wrote to a closed thread';
  exception when others then
    if sqlerrm <> 'support.closed' then raise exception 'FAIL: closed write: %', sqlerrm; end if;
  end;
  begin
    perform public.support_set_open(tid, true);
    raise exception 'FAIL: player reopened';
  exception when others then
    if sqlerrm <> 'support.notFound' then raise exception 'FAIL: player reopen: %', sqlerrm; end if;
  end;
  -- Günde 3 konu.
  t2 := public.support_open('İkinci', 'iki');
  perform public.support_open('Üçüncü', 'üç');
  begin
    perform public.support_open('Dördüncü', 'dört');
    raise exception 'FAIL: fourth thread in a day';
  exception when others then
    if sqlerrm <> 'friends.rateLimited' then raise exception 'FAIL: daily limit: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- admin yazınca yeniden açılır
  perform set_config('request.jwt.claims',
    json_build_object('sub', adm, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select x.admin into n from public.support_unread() x;
  perform public.support_send(tid, 'Bir şey daha soracaktım.');
  select x.status into r from public.admin_support_threads(true) x where x.id = tid;
  if r.status is distinct from 'open' then raise exception 'FAIL: admin reply did not reopen'; end if;
  perform public.support_set_open(t2, false);
  if not exists (select 1 from public.admin_support_threads(false) x where x.id = t2) then
    raise exception 'FAIL: closed list';
  end if;
  execute 'reset role';
  if not exists (select 1 from admin_log l where l.admin = adm and l.action = 'closeSupport'
                   and l.target = t2::text) then
    raise exception 'FAIL: admin close not logged';
  end if;

  -- ---------------------------------------------------------------- açık konu sınırı (5)
  insert into support_threads (owner, subject, created_at)
  select b, 'eski ' || i, now() - interval '3 days' from generate_series(1, 5) i;
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.support_open('Altıncı', 'altı');
    raise exception 'FAIL: sixth open thread';
  exception when others then
    if sqlerrm <> 'support.tooManyOpen' then raise exception 'FAIL: open limit: %', sqlerrm; end if;
  end;
  execute 'reset role';

  -- ---------------------------------------------------------------- banlı oyuncu itiraz edebilir
  perform set_config('request.jwt.claims',
    json_build_object('sub', c, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  t2 := public.support_open('Ban itirazı', 'Yanlışlıkla banlandım.');
  perform public.support_send(t2, 'Lütfen bakar mısınız?');
  execute 'reset role';
  perform set_config('request.jwt.claims',
    json_build_object('sub', adm, 'role', 'authenticated', 'is_anonymous', false)::text, true);
  execute 'set local role authenticated';
  select * into r from public.admin_support_threads(true) x where x.id = t2;
  if r.id is null or not r.banned or r.unread <> 2 then raise exception 'FAIL: banned appeal %', r; end if;
  execute 'reset role';

  -- ---------------------------------------------------------------- anonim kimlik yazamaz
  perform set_config('request.jwt.claims',
    json_build_object('sub', b, 'role', 'authenticated', 'is_anonymous', true)::text, true);
  execute 'set local role authenticated';
  begin
    perform public.support_open('Anonim', 'deneme');
    raise exception 'FAIL: anonymous identity opened a thread';
  exception when others then
    if sqlerrm <> 'support.membersOnly' then raise exception 'FAIL: anonymous: %', sqlerrm; end if;
  end;
  execute 'reset role';

  raise exception 'SUPPORT_OK all checks passed (nothing was saved)';
end;
$test$;
