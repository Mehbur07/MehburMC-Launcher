// MehburMC Launcher — phase 24 (K86): short-lived TURN credentials for voice calls.
//
// Deploy in Supabase → Edge Functions → "Deploy a new function" → "Via Editor",
// name it `turn-credentials` and paste this file. Secrets (Edge Functions →
// Secrets): CLOUDFLARE_TURN_KEY_ID and CLOUDFLARE_TURN_API_TOKEN from
// Cloudflare → Realtime → TURN Server. The secrets never leave the server.
//
// Only signed-in email accounts get credentials (checked against Supabase
// Auth, so this holds even if "Verify JWT" is turned off). Without the
// secrets the function answers with an empty list and calls use STUN only.

const TTL_SECONDS = 86400;

const json = (body: unknown, status = 200) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });

interface IceServer {
  urls: string | string[];
  username?: string;
  credential?: string;
}

// Browsers block port 53; such URLs only time out.
const usable = (url: string) => /^(stun|turns?):/.test(url) && !/:53(\?|$)/.test(url);

Deno.serve(async (req) => {
  if (req.method !== "POST") return json({ error: "method" }, 405);

  const auth = req.headers.get("Authorization") ?? "";
  const supabaseUrl = Deno.env.get("SUPABASE_URL");
  const anonKey = Deno.env.get("SUPABASE_ANON_KEY");
  if (!auth.startsWith("Bearer ") || !supabaseUrl || !anonKey) {
    return json({ error: "auth.required" }, 401);
  }
  const who = await fetch(`${supabaseUrl}/auth/v1/user`, {
    headers: { Authorization: auth, apikey: anonKey },
  });
  if (!who.ok) return json({ error: "auth.required" }, 401);
  const user = await who.json();
  if (user?.is_anonymous !== false || !user?.email) {
    return json({ error: "friends.disabled" }, 403);
  }

  const keyId = Deno.env.get("CLOUDFLARE_TURN_KEY_ID");
  const token = Deno.env.get("CLOUDFLARE_TURN_API_TOKEN");
  if (!keyId || !token) return json({ iceServers: [], ttl: 0 });

  const res = await fetch(
    `https://rtc.live.cloudflare.com/v1/turn/keys/${encodeURIComponent(keyId)}/credentials/generate-ice-servers`,
    {
      method: "POST",
      headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ ttl: TTL_SECONDS }),
    },
  );
  if (!res.ok) {
    console.error("cloudflare turn", res.status, await res.text());
    return json({ error: "calls.turnUnavailable" }, 502);
  }
  const body = await res.json();
  // Older responses carried a single object instead of an array.
  const raw: IceServer[] = Array.isArray(body?.iceServers)
    ? body.iceServers
    : body?.iceServers
      ? [body.iceServers]
      : [];
  const iceServers = raw
    .map((s) => ({
      ...s,
      urls: (Array.isArray(s.urls) ? s.urls : [s.urls]).filter(
        (u) => typeof u === "string" && usable(u),
      ),
    }))
    .filter((s) => s.urls.length > 0);
  return json({ iceServers, ttl: TTL_SECONDS });
});
