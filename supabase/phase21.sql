-- MehburMC Launcher — phase 21: the MehburMC skin/cape becomes private
-- (ARCHITECTURE K74). Run once in Supabase → SQL Editor after phase20.sql.
-- Safe to re-run.
--
-- The old public design is retired: its community shares are hidden and it
-- can never be shared again. New private textures live only on the server
-- (bucket `private_textures`, never in the repo); the founder (rank 1)
-- uploads them and grants them to chosen accounts. A granted texture shows
-- up in that user's launcher and leaves again when the grant is revoked.
-- Private textures cannot be shared to the community either.

create table if not exists public.retired_textures (
  sha1 text primary key check (sha1 ~ '^[0-9a-f]{40}$')
);
insert into public.retired_textures (sha1) values
  ('15f36107b3e370aa9b39b7327f421b0fc371f0b0'),  -- old MehburMC skin
  ('550a2572b75449fd44f33723a4bea217ba767805')   -- old MehburMC cape
on conflict do nothing;

create table if not exists public.private_textures (
  id bigint generated always as identity primary key,
  kind text not null check (kind in ('skin', 'cape')),
  model text not null default 'classic' check (model in ('classic', 'slim')),
  name text not null check (char_length(btrim(name)) between 1 and 48),
  sha1 text not null unique check (sha1 ~ '^[0-9a-f]{40}$'),
  created_by uuid references auth.users (id) on delete set null,
  created_at timestamptz not null default now()
);

create table if not exists public.texture_grants (
  texture_id bigint not null references public.private_textures (id) on delete cascade,
  user_id uuid not null references auth.users (id) on delete cascade,
  granted_by uuid references auth.users (id) on delete set null,
  created_at timestamptz not null default now(),
  primary key (texture_id, user_id)
);

alter table public.retired_textures enable row level security;
alter table public.private_textures enable row level security;
alter table public.texture_grants enable row level security;
revoke all on public.retired_textures, public.private_textures, public.texture_grants
  from anon, authenticated;

-- ---------------------------------------------------------------- no sharing

create or replace function public.texture_not_shareable()
returns trigger language plpgsql security definer set search_path = public as $$
begin
  if exists (select 1 from retired_textures r where r.sha1 = new.sha1)
     or exists (select 1 from private_textures p where p.sha1 = new.sha1) then
    raise exception 'textures.notShareable' using errcode = 'P0001';
  end if;
  return new;
end;
$$;
drop trigger if exists shared_textures_shareable on public.shared_textures;
create trigger shared_textures_shareable
  before insert or update of sha1 on public.shared_textures
  for each row execute function public.texture_not_shareable();

-- Existing community shares of the old design disappear.
update public.shared_textures t set hidden = true
 where t.sha1 in (select r.sha1 from public.retired_textures r) and not t.hidden;

-- ---------------------------------------------------------------- users

-- The caller's granted private textures (none while banned).
create or replace function public.my_private_textures()
returns table (id bigint, kind text, model text, name text, sha1 text)
language sql stable security definer set search_path = public as $$
  select p.id, p.kind, p.model, p.name, p.sha1
    from texture_grants g join private_textures p on p.id = g.texture_id
   where g.user_id = auth.uid() and public.is_member() and not public.is_banned(auth.uid())
   order by p.created_at;
$$;

-- Storage read check: the founder, or an account the texture is granted to.
create or replace function public.can_read_private_texture(p_path text)
returns boolean language sql stable security definer set search_path = public as $$
  select public.is_member() and not public.is_banned(auth.uid()) and (
    public.admin_level(auth.uid()) = 2
    or exists (select 1 from texture_grants g join private_textures p on p.id = g.texture_id
                where g.user_id = auth.uid() and p.sha1 || '.png' = p_path));
$$;

-- Storage write check: the founder only.
create or replace function public.can_write_private_texture()
returns boolean language sql stable security definer set search_path = public as $$
  select public.is_member() and not public.is_banned(auth.uid())
     and public.admin_level(auth.uid()) = 2;
$$;

-- ---------------------------------------------------------------- founder

create or replace function public.admin_private_textures()
returns table (id bigint, kind text, model text, name text, sha1 text,
               created_at timestamptz, grants bigint)
language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(2);
  return query
    select p.id, p.kind, p.model, p.name, p.sha1, p.created_at,
           (select count(*) from texture_grants g where g.texture_id = p.id)
      from private_textures p order by p.created_at desc;
end;
$$;

-- Records a texture uploaded to private_textures/<sha1>.png (same image
-- again = rename). Returns its id.
create or replace function public.admin_add_private_texture(
  p_kind text, p_model text, p_name text, p_sha1 text
) returns bigint language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(2);
  clean_sha text := lower(coalesce(p_sha1, ''));
  out_id bigint;
begin
  if exists (select 1 from retired_textures r where r.sha1 = clean_sha) then
    raise exception 'textures.notShareable' using errcode = 'P0001';
  end if;
  insert into private_textures as p (kind, model, name, sha1, created_by)
  values (p_kind, case when p_kind = 'cape' then 'classic' else coalesce(p_model, 'classic') end,
          btrim(p_name), clean_sha, me)
  on conflict (sha1) do update
    set kind = excluded.kind, model = excluded.model, name = excluded.name
  returning p.id into out_id;
  -- Anyone who already shared this image loses the share.
  update shared_textures t set hidden = true where t.sha1 = clean_sha and not t.hidden;
  perform public.admin_note(me, 'addPrivateTexture', out_id::text, p_name);
  return out_id;
end;
$$;

-- Deletes a private texture (grants go with it); returns its SHA-1 so the
-- launcher can delete the file.
create or replace function public.admin_delete_private_texture(p_id bigint)
returns text language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(2);
  out_sha text;
begin
  delete from private_textures p where p.id = p_id returning p.sha1 into out_sha;
  if out_sha is null then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  perform public.admin_note(me, 'deletePrivateTexture', p_id::text, out_sha);
  return out_sha;
end;
$$;

create or replace function public.admin_texture_grants(p_id bigint)
returns table (user_id uuid, names text[], created_at timestamptz)
language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(2);
  return query
    select g.user_id, public.user_names(g.user_id), g.created_at
      from texture_grants g where g.texture_id = p_id order by g.created_at;
end;
$$;

create or replace function public.admin_grant_texture(p_id bigint, p_user uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(2);
begin
  if not exists (select 1 from private_textures p where p.id = p_id) then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  if not exists (select 1 from auth.users u
                  where u.id = p_user and not coalesce(u.is_anonymous, false)) then
    raise exception 'admin.userNotFound' using errcode = 'P0001';
  end if;
  insert into texture_grants (texture_id, user_id, granted_by) values (p_id, p_user, me)
  on conflict do nothing;
  perform public.admin_note(me, 'grantTexture', p_id::text, p_user::text);
end;
$$;

create or replace function public.admin_revoke_texture(p_id bigint, p_user uuid)
returns void language plpgsql security definer set search_path = public as $$
declare me uuid := public.require_admin(2);
begin
  delete from texture_grants g where g.texture_id = p_id and g.user_id = p_user;
  perform public.admin_note(me, 'revokeTexture', p_id::text, p_user::text);
end;
$$;

revoke execute on function public.texture_not_shareable() from public, anon, authenticated;
grant execute on function
  public.my_private_textures(),
  public.can_read_private_texture(text),
  public.can_write_private_texture(),
  public.admin_private_textures(),
  public.admin_add_private_texture(text, text, text, text),
  public.admin_delete_private_texture(bigint),
  public.admin_texture_grants(bigint),
  public.admin_grant_texture(bigint, uuid),
  public.admin_revoke_texture(bigint, uuid)
to authenticated;

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('private_textures', 'private_textures', false, 131072, array['image/png'])
on conflict (id) do update
  set public = false, file_size_limit = 131072, allowed_mime_types = array['image/png'];

drop policy if exists private_textures_upload on storage.objects;
create policy private_textures_upload on storage.objects for insert to authenticated
  with check (bucket_id = 'private_textures' and public.can_write_private_texture());
drop policy if exists private_textures_update on storage.objects;
create policy private_textures_update on storage.objects for update to authenticated
  using (bucket_id = 'private_textures' and public.can_write_private_texture());
drop policy if exists private_textures_delete on storage.objects;
create policy private_textures_delete on storage.objects for delete to authenticated
  using (bucket_id = 'private_textures' and public.can_write_private_texture());
drop policy if exists private_textures_read on storage.objects;
create policy private_textures_read on storage.objects for select to authenticated
  using (bucket_id = 'private_textures' and public.can_read_private_texture(name));
