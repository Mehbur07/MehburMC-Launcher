-- MehburMC Launcher — MehburMC Kütüphane sitesi (mehburmclibrary.github.io) için
-- ekler (K102). Supabase → SQL Editor'de library_packs.sql'den sonra çalıştır,
-- sonra library_web_test.sql. Birden fazla çalıştırmak güvenlidir.
--
-- 1) Giriş yapmamış ziyaretçi (anon rolü) onaylı içeriği listeler ve indirir.
--    Listede sahibin kullanıcı kimliği sütunu, tarama raporu ve admin notları
--    yoktur. İndirme yolu (<sahip>/<sha1>.<uzantı>) dosyanın depodaki adı
--    olduğu için verilir.
-- 2) Her yüklemenin bir kaynağı vardır: 'launcher' (varsayılan) ya da 'web'.
--    Site submit_library_mod'u p_source => 'web' ile çağırır; adminler bunu
--    admin_library'de görür. Eski launcher'lar p_source göndermez → 'launcher'.
-- 3) Ziyaretçi, launcher'a ait library_list'i artık çağıramaz (sahip kimliği
--    ve tarama raporu döndürüyordu). Launcher her zaman oturum açtığı için
--    (anonim oturum dahil) bundan etkilenmez.

-- ---------------------------------------------------------------- kaynak

alter table public.library_mods
  add column if not exists source text not null default 'launcher';

alter table public.library_mods drop constraint if exists library_mods_source_check;
alter table public.library_mods add constraint library_mods_source_check
  check (source in ('launcher', 'web'));

-- library_packs.sql'deki submit_library_mod + p_source. Eski 12 parametreli
-- sürüm silinir; kalsaydı PostgREST hangisini çağıracağını seçemezdi.
drop function if exists public.submit_library_mod(text, text, text, text, text, text[], text,
                                                  text, bigint, text, jsonb, text);
create or replace function public.submit_library_mod(
  p_name text, p_description text, p_author text, p_mod_id text, p_version text,
  p_loaders text[], p_game_versions text, p_sha1 text, p_size bigint,
  p_file_name text, p_scan jsonb, p_kind text default 'mod', p_source text default 'launcher'
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(p_sha1, ''));
  kind text := coalesce(p_kind, 'mod');
  src text := coalesce(p_source, 'launcher');
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if kind not in ('mod', 'modpack', 'resourcepack', 'shader') then
    raise exception 'library.badKind' using errcode = 'P0001';
  end if;
  if src not in ('launcher', 'web') then
    raise exception 'library.badSource' using errcode = 'P0001';
  end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(p_author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  -- Rapor bu türden olmalı (tarayıcı 2+); eski raporlar mod sayılır.
  if coalesce(p_scan ->> 'verdict', '') not in ('pass', 'warn')
     or coalesce(p_scan ->> 'kind', 'mod') <> kind then
    raise exception 'library.blocked' using errcode = 'P0001';
  end if;
  if (select coalesce(sum(m.size), 0) from library_mods m
       where m.owner = me and m.sha1 <> clean_sha) + coalesce(p_size, 0) > 209715200 then
    raise exception 'library.storageFull' using errcode = 'P0001';
  end if;
  if not exists (select 1 from storage.objects o
                  where o.bucket_id = 'library'
                    and o.name = me::text || '/' || clean_sha || '.' || public.library_ext(kind)) then
    raise exception 'library.fileMissing' using errcode = 'P0001';
  end if;
  select m.id into existing from library_mods m where m.owner = me and m.sha1 = clean_sha;
  if existing is null then
    if (select count(*) from library_mods m where m.owner = me and m.status = 'pending') >= 10 then
      raise exception 'library.quota' using errcode = 'P0001';
    end if;
    if (select count(*) from library_mods m
         where m.owner = me and m.created_at > now() - interval '1 day') >= 10 then
      raise exception 'friends.rateLimited' using errcode = 'P0001';
    end if;
  end if;
  insert into library_mods as m (owner, author, name, description, mod_id, version, loaders,
                                 game_versions, sha1, size, file_name, scan, kind, source)
  values (me, btrim(p_author), btrim(p_name), btrim(coalesce(p_description, '')), p_mod_id,
          btrim(p_version), coalesce(p_loaders, '{}'), coalesce(p_game_versions, ''), clean_sha,
          p_size, p_file_name, p_scan, kind, src)
  on conflict on constraint library_mods_owner_sha1 do update
    set author = excluded.author, name = excluded.name, description = excluded.description,
        scan = excluded.scan, source = excluded.source, status = 'pending', review_note = null,
        reviewed_by = null, reviewed_at = null
  returning m.id into out_id;
  return out_id;
end;
$$;

-- library_packs.sql'deki admin_library + sondaki `source` sütunu. Dönen tablo
-- değiştiği için önce silinir; eski launcher'lar fazladan sütunu yok sayar.
drop function if exists public.admin_library(text);
create or replace function public.admin_library(p_status text)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, reports bigint, kind text, source text
) language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(case when p_status = 'pending' then 2 else 1 end);
  return query
    select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
           m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan, m.review_note,
           m.created_at,
           (select count(*) from library_reports r where r.library_mod = m.id),
           m.kind, m.source
      from library_mods m
     where m.status = p_status
     order by m.created_at desc
     limit 300;
end;
$$;

revoke execute on function
  public.submit_library_mod(text, text, text, text, text, text[], text, text, bigint, text, jsonb,
                            text, text),
  public.admin_library(text)
from public, anon;
grant execute on function
  public.submit_library_mod(text, text, text, text, text, text[], text, text, bigint, text, jsonb,
                            text, text),
  public.admin_library(text)
to authenticated;

-- ---------------------------------------------------------------- herkese açık liste

-- Onaylı bütün içerik, en son onaylanan başta. Sayfalama, arama ve filtreler
-- PostgREST'in kendi parametreleriyle yapılır (örn. kind=eq.mod, limit, offset,
-- loaders=cs.{fabric}); bu yüzden fonksiyonun kendi parametresi yok.
create or replace function public.library_public_list()
returns table (
  id bigint, kind text, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, file_path text, source text, created_at timestamptz, approved_at timestamptz
) language sql stable security definer set search_path = public as $$
  select m.id, m.kind, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
         m.game_versions, m.sha1, m.size, m.file_name,
         m.owner::text || '/' || m.sha1 || '.' || public.library_ext(m.kind),
         m.source, m.created_at, coalesce(m.reviewed_at, m.created_at)
    from library_mods m
   where m.status = 'approved'
   order by coalesce(m.reviewed_at, m.created_at) desc, m.id desc;
$$;

-- Depo okuma denetimi: yalnızca onaylı içeriğin dosyası. Giriş yapmış
-- kullanıcılar için library_packs.sql'deki can_read_library_file geçerli.
create or replace function public.library_file_is_public(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from library_mods m
     where m.status = 'approved'
       and m.owner::text || '/' || m.sha1 || '.' || public.library_ext(m.kind) = p_path
  );
$$;

revoke execute on function public.library_public_list(), public.library_file_is_public(text)
from public;
grant execute on function public.library_public_list(), public.library_file_is_public(text)
to anon, authenticated;

-- Ziyaretçi onaylı dosyayı indirir (imzalı bağlantı da bu izne bakar).
drop policy if exists library_public_read on storage.objects;
create policy library_public_read on storage.objects for select to anon
  using (bucket_id = 'library' and public.library_file_is_public(name));

-- Launcher'a ait liste ziyaretçiye kapalı (madde 3).
revoke execute on function public.library_list(int, boolean) from public, anon;
grant execute on function public.library_list(int, boolean) to authenticated;
