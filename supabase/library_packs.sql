-- MehburMC Launcher — MehburMC Library: modpacks, resource packs and shaders
-- (ARCHITECTURE K88). Run in Supabase → SQL Editor after phase24.sql, then
-- library_packs_test.sql. Safe to run again.
--
-- A library item now has a kind: 'mod' (.jar, up to 25 MiB), 'modpack'
-- (Modrinth .mrpack), 'resourcepack' or 'shader' (.zip), packs up to 50 MiB.
-- Files stay in the `library` bucket as <owner>/<sha1>.<jar|mrpack|zip>.
-- Each uploader may keep at most 200 MiB there (the free plan has 1 GB).
-- Launchers before K88 list mods only (library_list without p_all).

alter table public.library_mods
  add column if not exists kind text not null default 'mod';

alter table public.library_mods drop constraint if exists library_mods_kind_check;
alter table public.library_mods add constraint library_mods_kind_check
  check (kind in ('mod', 'modpack', 'resourcepack', 'shader'));

-- Packs need no loader; a mod still needs one.
alter table public.library_mods drop constraint if exists library_mods_loaders_check;
alter table public.library_mods add constraint library_mods_loaders_check
  check (cardinality(loaders) <= 4
         and loaders <@ array['fabric', 'quilt', 'forge', 'neoforge']
         and (kind <> 'mod' or cardinality(loaders) >= 1));

alter table public.library_mods drop constraint if exists library_mods_size_check;
alter table public.library_mods add constraint library_mods_size_check
  check (size >= 1 and size <= case when kind = 'mod' then 26214400 else 52428800 end);

alter table public.library_mods drop constraint if exists library_mods_file_name_check;
alter table public.library_mods add constraint library_mods_file_name_check
  check (file_name ~ ('^[A-Za-z0-9._+\-]{1,120}\.' || case kind when 'mod' then 'jar'
                                                                when 'modpack' then 'mrpack'
                                                                else 'zip' end || '$'));

create index if not exists library_mods_kind on public.library_mods (kind, status, created_at desc);

-- File extension of a kind in the bucket.
create or replace function public.library_ext(p_kind text)
returns text language sql immutable set search_path = public as $$
  select case p_kind when 'mod' then 'jar' when 'modpack' then 'mrpack' else 'zip' end;
$$;

-- ---------------------------------------------------------------- submit

drop function if exists public.submit_library_mod(text, text, text, text, text, text[], text,
                                                  text, bigint, text, jsonb);
create or replace function public.submit_library_mod(
  p_name text, p_description text, p_author text, p_mod_id text, p_version text,
  p_loaders text[], p_game_versions text, p_sha1 text, p_size bigint,
  p_file_name text, p_scan jsonb, p_kind text default 'mod'
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(p_sha1, ''));
  kind text := coalesce(p_kind, 'mod');
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if kind not in ('mod', 'modpack', 'resourcepack', 'shader') then
    raise exception 'library.badKind' using errcode = 'P0001';
  end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(p_author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  -- The report must be of this kind (scanner 2+); older reports are mods.
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
                                 game_versions, sha1, size, file_name, scan, kind)
  values (me, btrim(p_author), btrim(p_name), btrim(coalesce(p_description, '')), p_mod_id,
          btrim(p_version), coalesce(p_loaders, '{}'), coalesce(p_game_versions, ''), clean_sha,
          p_size, p_file_name, p_scan, kind)
  on conflict on constraint library_mods_owner_sha1 do update
    set author = excluded.author, name = excluded.name, description = excluded.description,
        scan = excluded.scan, status = 'pending', review_note = null,
        reviewed_by = null, reviewed_at = null
  returning m.id into out_id;
  return out_id;
end;
$$;

-- ---------------------------------------------------------------- lists

drop function if exists public.library_list(int);
create or replace function public.library_list(p_max int default 300, p_all boolean default false)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, mine boolean, kind text
) language sql stable security definer set search_path = public as $$
  select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
         m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan,
         case when m.owner = auth.uid() then m.review_note end,
         m.created_at, m.owner = auth.uid(), m.kind
    from library_mods m
   where (coalesce(p_all, false) or m.kind = 'mod')
     and (m.owner = auth.uid()
          or (m.status = 'approved'
              and not exists (select 1 from library_reports r
                               where r.library_mod = m.id and r.reporter = auth.uid())))
   order by m.owner = auth.uid() desc, m.created_at desc
   limit least(greatest(coalesce(p_max, 300), 1), 500);
$$;

drop function if exists public.admin_library(text);
create or replace function public.admin_library(p_status text)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, reports bigint, kind text
) language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(case when p_status = 'pending' then 2 else 1 end);
  return query
    select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
           m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan, m.review_note,
           m.created_at,
           (select count(*) from library_reports r where r.library_mod = m.id),
           m.kind
      from library_mods m
     where m.status = p_status
     order by m.created_at desc
     limit 300;
end;
$$;

-- Approving needs the admin launcher's own scan of the same file and kind.
create or replace function public.admin_review_library_mod(
  p_id bigint, p_approve boolean, p_note text, p_scan jsonb
) returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(2);
  m library_mods;
begin
  select * into m from library_mods x where x.id = p_id;
  if m.id is null then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  if p_approve and (coalesce(p_scan ->> 'sha1', '') <> m.sha1
                    or coalesce(p_scan ->> 'verdict', '') not in ('pass', 'warn')
                    or coalesce(p_scan ->> 'kind', 'mod') <> m.kind) then
    raise exception 'library.blocked' using errcode = 'P0001';
  end if;
  update library_mods x
     set status = case when p_approve then 'approved' else 'rejected' end,
         review_note = nullif(left(btrim(coalesce(p_note, '')), 500), ''),
         review_scan = case when p_approve then p_scan else x.review_scan end,
         reviewed_by = me, reviewed_at = now()
   where x.id = p_id;
  perform public.admin_note(me, case when p_approve then 'approveMod' else 'rejectMod' end,
                            p_id::text, m.name);
end;
$$;

-- Storage read check: own files, approved items, and admins for review.
create or replace function public.can_read_library_file(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from library_mods m
     where m.owner::text || '/' || m.sha1 || '.' || public.library_ext(m.kind) = p_path
       and (m.owner = auth.uid() or m.status = 'approved'
            or (public.is_member() and public.admin_level(auth.uid()) >= 1
                and not public.is_banned(auth.uid())))
  );
$$;

grant execute on function
  public.submit_library_mod(text, text, text, text, text, text[], text, text, bigint, text, jsonb, text),
  public.library_list(int, boolean),
  public.library_ext(text)
to authenticated;
grant execute on function public.admin_library(text) to authenticated;

-- ---------------------------------------------------------------- bucket

update storage.buckets
   set file_size_limit = 52428800,
       allowed_mime_types = array['application/java-archive', 'application/zip',
                                  'application/x-modrinth-modpack+zip']
 where id = 'library';
