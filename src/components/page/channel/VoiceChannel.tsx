import Enclave from "@/app/app";
import { ChannelPageProps } from "../PageView";
import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";

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

    if (channel.kind !== "voice" || !appRef.current?.server) return;

    appRef.current.server.voiceJoin = (pin, channelId) => {
      invoke("connect_to_vc", { hostname, pin, channelId }).catch(
        console.error,
      );
    };

    appRef.current.server.websocket?.send({
      method: "JoinVoice",
      channel_id: channel.id,
    });
  }, [channel.id]);

  const speakers =
    appRef.current?.server?.voiceChatSpeakers &&
    Object.keys(appRef.current?.server?.voiceChatSpeakers);

  const users = appRef.current?.server?.voiceChatUsers[channel.id];

  const usersLength = users?.length || 0;

  // Dynamic grid column layout based on user count
  const getGridCols = () => {
    if (usersLength <= 3) return "grid-cols-1";
    if (usersLength <= 6) return "grid-cols-3";
    if (usersLength <= 9) return "grid-cols-3";
    return "grid-cols-5";
  };

  // Avatar sizing scales up for fewer participants
  const getAvatarSize = () => {
    if (usersLength <= 2) return "h-24 w-24 text-3xl";
    if (usersLength <= 4) return "h-20 w-20 text-2xl";
    return "h-14 w-14 text-lg";
  };

  return (
    <div className="flex flex-col h-screen gap-2">
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border">
        <h2>{channel.name}</h2>
      </header>
      <div
        className={cn(
          "grid p-4 w-full gap-4 h-full auto-rows-fr bg-background/50 rounded-xl overflow-y-auto",
          getGridCols(),
        )}
      >
        {users?.map((pubkey) => {
          const user = appRef.current?.server?.users[pubkey];
          const isSpeaking = speakers?.includes(pubkey);
          const displayName = user?.displayName || "Unknown User";
          const fallbackLetter = displayName.charAt(0).toUpperCase();

          return (
            <div
              key={pubkey}
              className={cn(
                "relative flex flex-col items-center justify-center p-4 rounded-xl bg-card/80 transition-all duration-150 overflow-hidden group h-full",
                isSpeaking
                  ? "ring-2 ring-emerald-500 shadow-[0_0_15px_rgba(16,185,129,0.25)] bg-card"
                  : "ring-1 ring-border/60 hover:ring-border",
              )}
            >
              {/* Avatar Container */}
              <div className="relative flex items-center justify-center">
                <Avatar
                  className={cn("transition-all duration-200", getAvatarSize())}
                >
                  <AvatarImage
                    src={user?.avatar}
                    alt={displayName}
                    className="object-cover"
                  />
                  <AvatarFallback className="bg-muted font-bold text-muted-foreground">
                    {fallbackLetter}
                  </AvatarFallback>
                </Avatar>

                {/* Mute indicator badge overlay */}
                {/*{user?.isMuted && (
                  <div className="absolute -bottom-1 -right-1 p-1.5 bg-destructive rounded-full text-destructive-foreground shadow-md">
                    <MicOff className="w-3.5 h-3.5" />
                  </div>
                )}*/}
              </div>

              <div className="absolute bottom-3 left-3 max-w-[85%] truncate bg-background/80 backdrop-blur-md text-foreground py-1 px-2.5 rounded-md border border-border/50 text-xs font-medium shadow-sm">
                {displayName}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
