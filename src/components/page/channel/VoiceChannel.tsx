import Enclave from "@/app/app";
import { ChannelPageProps } from "../PageView";
import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

export default function VoiceChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const channel = appRef.current?.page?.channel;
  if (!channel) return null;

  const lastChannelId = useRef<string | null>(null);

  useEffect(() => {
    const hostname = appRef.current?.server?.hostname;
    if (!hostname) return;
    if (lastChannelId.current === channel.id) return;
    lastChannelId.current = channel.id;

    invoke("disconnect_from_vc");
    invoke("connect_to_vc", { hostname });
  }, [channel.id]);

  return (
    <div className="flex flex-col h-screen gap-2">
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border">
        <h2>{channel.name}</h2>
      </header>
    </div>
  );
}
