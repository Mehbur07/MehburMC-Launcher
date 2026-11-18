-- MehburMC Launcher — MehburMC Kütüphane sitesinin hesap sayfası için ek (K103).
-- Supabase → SQL Editor'de (launcher'ın projesi: rcaifwsiutoxilpkceht) çalıştır,
-- sonra site_account_test.sql. Birden fazla çalıştırmak güvenlidir.
--
-- Launcher hesap adlarını bilgisayarda tutar; site ise bunları bilemez. Site,
-- giriş yapan kullanıcıya kendi adlarını göstermek ve yüklemede "yükleyen"
-- adını seçtirmek için my_account_names() fonksiyonunu kullanır. Yalnızca
-- çağıranın kendi adları döner; başkasının adları ve ziyaretçi (anon) kapalı.
-- Ad ekleme için launcher'ın claim_name(name) fonksiyonu kullanılır.

create or replace function public.my_account_names()
returns table (name text, created_at timestamptz)
language sql stable security definer set search_path = public as $$
  select n.name, n.created_at
    from account_names n
   where n.owner = auth.uid()
   order by n.created_at, n.name_lower;
$$;

revoke execute on function public.my_account_names() from public, anon;
grant execute on function public.my_account_names() to authenticated;
