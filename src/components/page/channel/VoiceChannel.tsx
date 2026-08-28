import Enclave from "@/app/app";
import { ChannelPageProps } from "../PageView";
import { cn } from "@/lib/utils";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Phone, Volume2 } from "lucide-react";

export default function VoiceChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const channel = appRef.current?.page?.channel;

  if (!channel) return null;

  const server = appRef.current?.server;
  const isConnected = server?.voiceChannelId === channel.id;

  const speakers =
    server?.voiceChatSpeakers && Object.keys(server.voiceChatSpeakers);

  const users = server?.voiceChatUsers[channel.id];
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

  const handleJoinVoice = () => {
    appRef.current?.joinVoice(channel);
  };

  return (
    <div className="relative flex flex-col h-screen gap-2 pb-3 overflow-hidden">
      {/* Header */}
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border flex justify-between items-center z-10">
        <h2>{channel.name}</h2>
        <span className="text-xs">
          {usersLength} {usersLength === 1 ? "Participant" : "Participants"}
        </span>
      </header>

      {/* Main Grid Container */}
      <div className="relative flex-1 w-full min-h-0 overflow-hidden rounded-xl">
        {/* Active Grid - Fully readable at all times */}
        <div
          className={cn(
            "grid p-4 w-full h-full gap-4 auto-rows-fr bg-background/40 rounded-xl overflow-y-auto transition-opacity duration-300",
            getGridCols(),
          )}
        >
          {users?.map((pubkey) => {
            const user = server?.users[pubkey];
            const isSpeaking = speakers?.includes(pubkey);
            const displayName = user?.displayName || "Unknown User";
            const fallbackLetter = displayName.charAt(0).toUpperCase();

            return (
              <div
                key={pubkey}
                className={cn(
                  "relative flex flex-col items-center justify-center p-4 rounded-xl bg-card/90 transition-all duration-150 overflow-hidden group h-full",
                  isSpeaking
                    ? "ring-2 ring-emerald-500 shadow-[0_0_15px_rgba(16,185,129,0.25)] bg-card"
                    : "ring-1 ring-border/60 hover:ring-border",
                )}
              >
                {/* Avatar Container */}
                <div className="relative flex items-center justify-center">
                  <Avatar
                    className={cn(
                      "transition-all duration-200",
                      getAvatarSize(),
                    )}
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
                </div>

                {/* Name Plate */}
                <div className="absolute bottom-3 left-3 max-w-[85%] truncate bg-background/80 backdrop-blur-md text-foreground py-1 px-2.5 rounded-md border border-border/50 text-xs font-medium shadow-sm">
                  {displayName}
                </div>
              </div>
            );
          })}
        </div>

        {/* Floating Join CTA Overlay */}
        {!isConnected && (
          <div className="absolute inset-0 z-20 flex items-center justify-center bg-black/35 backdrop-brightness-75 pointer-events-auto transition-all animate-in fade-in duration-200">
            <div className="flex flex-col items-center gap-4 p-6 sm:p-8 rounded-2xl text-center max-w-xs sm:max-w-sm mx-4 transform scale-100 transition-all">
              <div className="p-3.5 rounded-full bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
                <Volume2 className="h-7 w-7 animate-pulse" />
              </div>

              <div className="space-y-1">
                <h3 className="text-base sm:text-lg font-semibold text-foreground">
                  Ready to join {channel.name}?
                </h3>
                <p className="text-xs text-muted-foreground">
                  {usersLength > 0
                    ? `${usersLength} ${usersLength === 1 ? "participant is" : "participants are"} currently connected.`
                    : "Channel is currently empty."}
                </p>
              </div>

              <Button
                onClick={handleJoinVoice}
                size="lg"
                className="w-full bg-emerald-600 hover:bg-emerald-500 text-white font-medium shadow-lg shadow-emerald-600/20 transition-all"
              >
                <Phone />
                Join Voice
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
