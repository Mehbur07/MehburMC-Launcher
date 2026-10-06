-- MehburMC Launcher — phase 19: MehburMC Library (community mods, published
-- after admin approval; ARCHITECTURE K72). Run once in Supabase → SQL Editor
-- after phase17.sql. Safe to re-run.

-- One row per uploaded mod. The jar lives in the private `library` bucket as
-- <owner>/<sha1>.jar and is uploaded before the row is created. Rows start
-- as 'pending'; admins (phase 20) approve or reject them.
create table if not exists public.library_mods (
  id bigint generated always as identity primary key,
  owner uuid not null references auth.users (id) on delete cascade,
  author text not null check (author ~ '^[A-Za-z0-9_]{3,16}$'),
  name text not null check (char_length(btrim(name)) between 1 and 64),
  description text not null default '' check (char_length(description) <= 1000),
  mod_id text not null check (mod_id ~ '^[a-z0-9_.\-]{1,64}$'),
  version text not null check (char_length(version) between 1 and 64),
  loaders text[] not null check (
    cardinality(loaders) between 1 and 4
    and loaders <@ array['fabric', 'quilt', 'forge', 'neoforge']),
  game_versions text not null default '' check (char_length(game_versions) <= 100),
  sha1 text not null check (sha1 ~ '^[0-9a-f]{40}$'),
  size bigint not null check (size between 1 and 26214400),
  file_name text not null check (file_name ~ '^[A-Za-z0-9._+\-]{1,120}\.jar$'),
  status text not null default 'pending' check (status in ('pending', 'approved', 'rejected')),
  -- The uploader's automatic check (verdict 'pass' or 'warn', findings).
  scan jsonb not null,
  review_note text check (char_length(review_note) <= 500),
  reviewed_by uuid references auth.users (id) on delete set null,
  reviewed_at timestamptz,
  created_at timestamptz not null default now(),
  constraint library_mods_owner_sha1 unique (owner, sha1)
);
create index if not exists library_mods_status on public.library_mods (status, created_at desc);

create table if not exists public.library_reports (
  id bigint generated always as identity primary key,
  library_mod bigint not null references public.library_mods (id) on delete cascade,
  reporter uuid not null references auth.users (id) on delete cascade,
  reason text not null check (reason in ('malware', 'broken', 'stolen', 'other')),
  note text check (char_length(note) <= 500),
  created_at timestamptz not null default now(),
  unique (library_mod, reporter)
);

alter table public.library_mods enable row level security;
alter table public.library_reports enable row level security;
revoke all on public.library_mods, public.library_reports from anon, authenticated;

-- Submits (or resubmits) the caller's mod for review. Returns the row id.
create or replace function public.submit_library_mod(
  p_name text, p_description text, p_author text, p_mod_id text, p_version text,
  p_loaders text[], p_game_versions text, p_sha1 text, p_size bigint,
  p_file_name text, p_scan jsonb
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(p_sha1, ''));
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(p_author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  if coalesce(p_scan ->> 'verdict', '') not in ('pass', 'warn') then
    raise exception 'library.blocked' using errcode = 'P0001';
  end if;
  if not exists (select 1 from storage.objects o
                  where o.bucket_id = 'library' and o.name = me::text || '/' || clean_sha || '.jar') then
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
                                 game_versions, sha1, size, file_name, scan)
  values (me, btrim(p_author), btrim(p_name), btrim(coalesce(p_description, '')), p_mod_id,
          btrim(p_version), p_loaders, coalesce(p_game_versions, ''), clean_sha, p_size,
          p_file_name, p_scan)
  on conflict on constraint library_mods_owner_sha1 do update
    set author = excluded.author, name = excluded.name, description = excluded.description,
        scan = excluded.scan, status = 'pending', review_note = null,
        reviewed_by = null, reviewed_at = null
  returning m.id into out_id;
  return out_id;
end;
$$;

-- Withdraws one of the caller's uploads; returns its SHA-1 so the launcher
-- can delete the file.
create or replace function public.withdraw_library_mod(p_id bigint)
returns text language plpgsql security definer set search_path = public as $$
declare out_sha text;
begin
  delete from library_mods m where m.id = p_id and m.owner = auth.uid()
  returning m.sha1 into out_sha;
  if out_sha is null then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  return out_sha;
end;
$$;

-- Approved mods (minus the ones the caller reported) and all of the
-- caller's own uploads.
create or replace function public.library_list(p_max int default 300)
returns table (
  id bigint, owner uuid, author text, name text, description text, mod_id text,
  version text, loaders text[], game_versions text, sha1 text, size bigint,
  file_name text, status text, scan jsonb, review_note text,
  created_at timestamptz, mine boolean
) language sql stable security definer set search_path = public as $$
  select m.id, m.owner, m.author, m.name, m.description, m.mod_id, m.version, m.loaders,
         m.game_versions, m.sha1, m.size, m.file_name, m.status, m.scan,
         case when m.owner = auth.uid() then m.review_note end,
         m.created_at, m.owner = auth.uid()
    from library_mods m
   where m.owner = auth.uid()
      or (m.status = 'approved'
          and not exists (select 1 from library_reports r
                           where r.library_mod = m.id and r.reporter = auth.uid()))
   order by m.owner = auth.uid() desc, m.created_at desc
   limit least(greatest(coalesce(p_max, 300), 1), 500);
$$;

create or replace function public.report_library_mod(p_id bigint, p_reason text, p_note text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  m library_mods;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  select * into m from library_mods x where x.id = p_id;
  if m.id is null or m.status <> 'approved' then
    raise exception 'library.notFound' using errcode = 'P0001';
  end if;
  if m.owner = me then
    raise exception 'textures.ownReport' using errcode = 'P0001';
  end if;
  if (select count(*) from library_reports r
       where r.reporter = me and r.created_at > now() - interval '1 hour') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into library_reports (library_mod, reporter, reason, note)
  values (p_id, me, p_reason, nullif(btrim(coalesce(p_note, '')), ''))
  on conflict do nothing;
end;
$$;

-- Storage read check: own files and files of approved mods.
create or replace function public.can_read_library_file(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from library_mods m
     where m.owner::text || '/' || m.sha1 || '.jar' = p_path
       and (m.owner = auth.uid() or m.status = 'approved')
  );
$$;

grant execute on function
  public.submit_library_mod(text, text, text, text, text, text[], text, text, bigint, text, jsonb),
  public.withdraw_library_mod(bigint),
  public.library_list(int),
  public.report_library_mod(bigint, text, text),
  public.can_read_library_file(text)
to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('library', 'library', false, 26214400, array['application/java-archive'])
on conflict (id) do update
  set public = false, file_size_limit = 26214400,
      allowed_mime_types = array['application/java-archive'];

drop policy if exists library_upload on storage.objects;
create policy library_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_update on storage.objects;
create policy library_update on storage.objects for update to authenticated
  using (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_delete on storage.objects;
create policy library_delete on storage.objects for delete to authenticated
  using (bucket_id = 'library' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists library_read on storage.objects;
create policy library_read on storage.objects for select to authenticated
  using (bucket_id = 'library' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.can_read_library_file(name)));
