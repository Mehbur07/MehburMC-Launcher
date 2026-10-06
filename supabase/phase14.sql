-- MehburMC Launcher — phase 14: account names unique across all users
-- (ARCHITECTURE K67). Run once in Supabase → SQL Editor after phase13.sql.
-- Safe to re-run.

-- One row per reserved name. Owned by the installation's anonymous
-- identity, not by the friends profile: turning friends off keeps names.
create table if not exists public.account_names (
  name_lower text primary key check (name_lower ~ '^[a-z0-9_]{3,16}$'),
  name text not null check (name ~ '^[A-Za-z0-9_]{3,16}$' and lower(name) = name_lower),
  owner uuid not null references auth.users (id) on delete cascade,
  created_at timestamptz not null default now()
);
create index if not exists account_names_owner on public.account_names (owner);

-- No policies: the table is reachable only through the functions below, so
-- nobody can list other people's names.
alter table public.account_names enable row level security;
revoke all on public.account_names from anon, authenticated;

-- Reserves `name` for the caller. Same owner = no-op (spelling updated).
create or replace function public.claim_name(name text)
returns void language plpgsql security definer set search_path = public as $$
declare
  me uuid := auth.uid();
  clean text := btrim(coalesce(name, ''));
  holder uuid;
begin
  if me is null then raise exception 'not signed in' using errcode = '28000'; end if;
  if clean !~ '^[A-Za-z0-9_]{3,16}$' then
    raise exception 'account.nameInvalid' using errcode = 'P0001';
  end if;
  select owner into holder from account_names where name_lower = lower(clean);
  if holder = me then
    update account_names set name = clean where name_lower = lower(clean);
    return;
  elsif holder is not null then
    raise exception 'account.nameTaken' using errcode = 'P0001';
  end if;
  if (select count(*) from account_names where owner = me) >= 10 then
    raise exception 'account.nameLimit' using errcode = 'P0001';
  end if;
  insert into account_names (name_lower, name, owner) values (lower(clean), clean, me)
  on conflict (name_lower) do nothing;
  if not found then
    raise exception 'account.nameTaken' using errcode = 'P0001';
  end if;
end;
$$;

create or replace function public.release_name(name text)
returns void language sql security definer set search_path = public as $$
  delete from account_names where name_lower = lower(btrim(name)) and owner = auth.uid();
$$;

-- Atomic rename: one transaction, so if the new name is taken the old one
-- is kept (the delete is rolled back with the error).
create or replace function public.rename_name(old_name text, new_name text)
returns void language plpgsql security definer set search_path = public as $$
begin
  if lower(btrim(old_name)) <> lower(btrim(new_name)) then
    -- Frees a slot first so a rename never trips the per-identity limit.
    delete from account_names where name_lower = lower(btrim(old_name)) and owner = auth.uid();
  end if;
  perform claim_name(new_name);
end;
$$;

-- Startup sync of existing local accounts: claims what it can and reports
-- each name as 'ok', 'taken' or 'limit'.
create or replace function public.claim_names(names text[])
returns table (name text, result text)
language plpgsql security definer set search_path = public as $$
declare n text;
begin
  foreach n in array coalesce(names, '{}') loop
    begin
      perform claim_name(n);
      name := n; result := 'ok';
    exception when others then
      name := n;
      result := case sqlerrm when 'account.nameLimit' then 'limit'
                             when 'account.nameInvalid' then 'invalid'
                             else 'taken' end;
    end;
    return next;
  end loop;
end;
$$;

-- "Turn off friends and delete my data": profile, friendships, messages and
-- lists go (cascade); the identity and its reserved names stay.
create or replace function public.delete_friend_data()
returns void language sql security definer set search_path = public as $$
  delete from profiles where id = auth.uid();
$$;

grant execute on function public.claim_name(text), public.release_name(text),
  public.rename_name(text, text), public.claim_names(text[]), public.delete_friend_data()
  to authenticated;
