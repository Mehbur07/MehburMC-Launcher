-- MehburMC Launcher — phase 17: share your own skins and capes (ARCHITECTURE
-- K70). Run once in Supabase → SQL Editor after phase14.sql. Safe to re-run.

-- One row per shared texture. The PNG lives in the private `textures`
-- bucket as <owner>/<sha1>.png and is uploaded before the row is created.
create table if not exists public.shared_textures (
  id bigint generated always as identity primary key,
  owner uuid not null references auth.users (id) on delete cascade,
  kind text not null check (kind in ('skin', 'cape')),
  model text not null default 'classic' check (model in ('classic', 'slim')),
  name text not null check (char_length(btrim(name)) between 1 and 48),
  -- One of the owner's reserved account names (account_names, K67).
  author text not null check (author ~ '^[A-Za-z0-9_]{3,16}$'),
  sha1 text not null check (sha1 ~ '^[0-9a-f]{40}$'),
  visibility text not null check (visibility in ('public', 'friends')),
  -- Set by admins (phase 20); hidden rows are visible to nobody but the owner.
  hidden boolean not null default false,
  created_at timestamptz not null default now(),
  unique (owner, sha1)
);
create index if not exists shared_textures_recent on public.shared_textures (created_at desc);

-- "Report" from other users; the admin view comes with phase 20.
create table if not exists public.texture_reports (
  id bigint generated always as identity primary key,
  texture_id bigint not null references public.shared_textures (id) on delete cascade,
  reporter uuid not null references auth.users (id) on delete cascade,
  reason text not null check (reason in ('inappropriate', 'stolen', 'spam', 'other')),
  note text check (char_length(note) <= 500),
  created_at timestamptz not null default now(),
  unique (texture_id, reporter)
);

-- Reachable only through the functions below.
alter table public.shared_textures enable row level security;
alter table public.texture_reports enable row level security;
revoke all on public.shared_textures, public.texture_reports from anon, authenticated;

-- Shares (or updates) the caller's texture. Returns the row id.
create or replace function public.share_texture(
  kind text, model text, name text, author text, sha1 text, visibility text
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean_sha text := lower(coalesce(sha1, ''));
  existing bigint;
  out_id bigint;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if not exists (select 1 from account_names a
                  where a.owner = me and a.name_lower = lower(btrim(coalesce(author, '')))) then
    raise exception 'textures.authorInvalid' using errcode = 'P0001';
  end if;
  if not exists (select 1 from storage.objects o
                  where o.bucket_id = 'textures' and o.name = me::text || '/' || clean_sha || '.png') then
    raise exception 'textures.fileMissing' using errcode = 'P0001';
  end if;
  select t.id into existing from shared_textures t where t.owner = me and t.sha1 = clean_sha;
  if existing is null then
    if (select count(*) from shared_textures t where t.owner = me) >= 30 then
      raise exception 'textures.quota' using errcode = 'P0001';
    end if;
    if (select count(*) from shared_textures t
         where t.owner = me and t.created_at > now() - interval '1 hour') >= 10 then
      raise exception 'friends.rateLimited' using errcode = 'P0001';
    end if;
  end if;
  insert into shared_textures as t (owner, kind, model, name, author, sha1, visibility)
  values (me, kind, coalesce(model, 'classic'), btrim(name), btrim(author), clean_sha, visibility)
  on conflict (owner, sha1) do update
    set kind = excluded.kind, model = excluded.model, name = excluded.name,
        author = excluded.author, visibility = excluded.visibility
  returning t.id into out_id;
  return out_id;
end;
$$;

-- Withdraws the caller's share; returns its SHA-1 so the launcher can
-- delete the file (SQL may not delete storage objects).
create or replace function public.unshare_texture(texture_id bigint)
returns text language plpgsql security definer set search_path = public as $$
declare out_sha text;
begin
  delete from shared_textures t where t.id = texture_id and t.owner = auth.uid()
  returning t.sha1 into out_sha;
  if out_sha is null then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  return out_sha;
end;
$$;

-- Whether the caller may see a share.
create or replace function public.can_see_texture(t public.shared_textures)
returns boolean language sql stable security definer set search_path = public as $$
  select t.owner = auth.uid()
      or (not t.hidden and (t.visibility = 'public'
          or (t.visibility = 'friends' and are_friends(auth.uid(), t.owner))));
$$;

-- Shares the caller can see (own first), minus the ones they reported.
create or replace function public.community_textures(max_rows int default 300)
returns table (
  id bigint, owner uuid, kind text, model text, name text, author text,
  sha1 text, visibility text, created_at timestamptz, mine boolean
) language sql stable security definer set search_path = public as $$
  select t.id, t.owner, t.kind, t.model, t.name, t.author, t.sha1, t.visibility,
         t.created_at, t.owner = auth.uid()
    from shared_textures t
   where can_see_texture(t)
     and not exists (select 1 from texture_reports r
                      where r.texture_id = t.id and r.reporter = auth.uid())
   order by t.owner = auth.uid() desc, t.created_at desc
   limit least(greatest(coalesce(max_rows, 300), 1), 500);
$$;

create or replace function public.report_texture(texture_id bigint, reason text, note text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  t shared_textures;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  select * into t from shared_textures s where s.id = texture_id;
  if t.id is null or not can_see_texture(t) then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  if t.owner = me then
    raise exception 'textures.ownReport' using errcode = 'P0001';
  end if;
  if (select count(*) from texture_reports r
       where r.reporter = me and r.created_at > now() - interval '1 hour') >= 20 then
    raise exception 'friends.rateLimited' using errcode = 'P0001';
  end if;
  insert into texture_reports (texture_id, reporter, reason, note)
  values (texture_id, me, reason, nullif(btrim(coalesce(note, '')), ''))
  on conflict do nothing;
end;
$$;

-- Storage read check: `<owner>/<sha1>.png` of a share the caller can see.
create or replace function public.can_read_texture_file(path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from shared_textures t
     where t.owner::text || '/' || t.sha1 || '.png' = path and can_see_texture(t)
  );
$$;

revoke all on function public.can_see_texture(public.shared_textures) from public, anon, authenticated;
grant execute on function
  public.share_texture(text, text, text, text, text, text),
  public.unshare_texture(bigint),
  public.community_textures(int),
  public.report_texture(bigint, text, text),
  public.can_read_texture_file(text)
to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('textures', 'textures', false, 131072, array['image/png'])
on conflict (id) do update
  set public = false, file_size_limit = 131072, allowed_mime_types = array['image/png'];

drop policy if exists textures_upload on storage.objects;
create policy textures_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_update on storage.objects;
create policy textures_update on storage.objects for update to authenticated
  using (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_delete on storage.objects;
create policy textures_delete on storage.objects for delete to authenticated
  using (bucket_id = 'textures' and (storage.foldername(name))[1] = auth.uid()::text);
drop policy if exists textures_read on storage.objects;
create policy textures_read on storage.objects for select to authenticated
  using (bucket_id = 'textures' and (
    (storage.foldername(name))[1] = auth.uid()::text
    or public.can_read_texture_file(name)));
