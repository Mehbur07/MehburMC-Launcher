-- MehburMC Launcher — phase 17 fix: `sha1` was ambiguous (parameter vs
-- column) in share_texture's ON CONFLICT. Run once in SQL Editor; safe to re-run.

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
  on conflict on constraint shared_textures_owner_sha1_key do update
    set kind = excluded.kind, model = excluded.model, name = excluded.name,
        author = excluded.author, visibility = excluded.visibility
  returning t.id into out_id;
  return out_id;
end;
$$;
