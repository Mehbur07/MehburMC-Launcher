-- MehburMC Launcher — phase 21 fix 1: community moderation in the Admin
-- panel (ARCHITECTURE K74). Run once in Supabase → SQL Editor after
-- phase21.sql. Safe to re-run.
--
-- The founder (rank 1) lists every shared skin/cape — public, friends-only
-- and already removed ones — removes them (admin_hide_texture, phase 20)
-- and can bring a removed one back. The founder may also read the image
-- files so the list shows previews.

-- Every share, newest first; p_hidden picks removed (true) or listed ones.
create or replace function public.admin_textures(p_hidden boolean default false)
returns table (
  id bigint, owner uuid, kind text, model text, name text, author text,
  sha1 text, visibility text, created_at timestamptz, hidden boolean
) language plpgsql stable security definer set search_path = public as $$
begin
  perform public.require_admin(2);
  return query
    select t.id, t.owner, t.kind, t.model, t.name, t.author, t.sha1, t.visibility,
           t.created_at, t.hidden
      from shared_textures t
     where t.hidden = coalesce(p_hidden, false)
     order by t.created_at desc
     limit 500;
end;
$$;

-- Brings a removed share back (never a retired or private design).
create or replace function public.admin_restore_texture(p_id bigint)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := public.require_admin(2);
  s text;
begin
  select t.sha1 into s from shared_textures t where t.id = p_id;
  if s is null then
    raise exception 'textures.notFound' using errcode = 'P0001';
  end if;
  if exists (select 1 from retired_textures r where r.sha1 = s)
     or exists (select 1 from private_textures p where p.sha1 = s) then
    raise exception 'textures.notShareable' using errcode = 'P0001';
  end if;
  update shared_textures t set hidden = false where t.id = p_id;
  perform public.admin_note(me, 'restoreTexture', p_id::text, '');
end;
$$;

-- Image files: the owner (storage policy), anyone who can see the share,
-- or the founder.
create or replace function public.can_read_texture_file(path text)
returns boolean language sql stable security definer set search_path = public as $$
  select exists (
    select 1 from shared_textures t
     where t.owner::text || '/' || t.sha1 || '.png' = path
       and (can_see_texture(t)
            or (public.admin_level(auth.uid()) >= 2 and not public.is_banned(auth.uid())))
  );
$$;

grant execute on function
  public.admin_textures(boolean),
  public.admin_restore_texture(bigint)
to authenticated;
