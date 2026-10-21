import { Phone, PhoneCall, Volume2 } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button, IconButton } from "../../components/ui";
import { useCalls } from "../../stores/calls";

/** "Voice call" in a friend's chat header (K86). */
export function FriendCallButton({ friendId }: { friendId: string }) {
  const { t } = useTranslation();
  const start = useCalls((s) => s.start);
  const joining = useCalls((s) => s.joining);
  const inCall = useCalls((s) => {
    const c = s.calls.find((x) => x.id === s.activeId);
    return c?.groupId === null && c.members.some((m) => m.id === friendId);
  });
  return (
    <IconButton
      label={inCall ? t("calls.inCall") : t("calls.call")}
      disabled={inCall || joining}
      onClick={() => void start({ friend: friendId })}
      className={inCall ? "text-online" : "hover:text-online"}
    >
      <Phone size={15} />
    </IconButton>
  );
}

/** Start or join the group's voice chat (K86). */
export function GroupCallButton({ groupId }: { groupId: number }) {
  const { t } = useTranslation();
  const start = useCalls((s) => s.start);
  const joining = useCalls((s) => s.joining);
  const live = useCalls((s) => s.calls.find((c) => c.groupId === groupId));
  const activeId = useCalls((s) => s.activeId);
  const joined = live?.members.filter((m) => m.state === "joined").length ?? 0;
  const inCall = live !== undefined && live.id === activeId;
  if (inCall) {
    return (
      <span className="flex items-center gap-1 px-2 text-xs font-semibold text-online">
        <Volume2 size={14} />
        {t("calls.inCall")}
      </span>
    );
  }
  return (
    <Button
      size="sm"
      variant={live ? "primary" : "secondary"}
      disabled={joining}
      onClick={() => void start({ group: groupId })}
    >
      <PhoneCall size={13} />
      {live ? t("calls.joinGroup", { count: joined }) : t("calls.startGroup")}
    </Button>
  );
}

/** Small marker in the group list while the group's voice chat is on. */
export function GroupLiveMark({ groupId }: { groupId: number }) {
  const { t } = useTranslation();
  const live = useCalls((s) => s.calls.some((c) => c.groupId === groupId));
  if (!live) return null;
  return (
    <Volume2
      size={13}
      className="shrink-0 text-online"
      role="img"
      aria-label={t("calls.groupLive")}
    />
  );
}
