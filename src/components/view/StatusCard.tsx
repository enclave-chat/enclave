import { Card } from "../ui/card";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";
import {
  CopyIcon,
  Headset,
  Mic,
  MicOff,
  PhoneOff,
  Settings2Icon,
  Signal,
} from "lucide-react";
import { Button } from "../ui/button";
import Enclave from "@/app/app";
import { base58 } from "@scure/base";
import * as ed from "@noble/ed25519";
import { Channel } from "@/lib/types";
import { updateBackendConfig } from "@/lib/config";

function findChannel(channels: Channel[], id: string): Channel | undefined {
  for (const channel of channels) {
    if (channel.id === id) return channel;

    if (channel.kind === "category") {
      const found = findChannel(channel.channels, id);
      if (found) return found;
    }
  }

  return undefined;
}

export default function StatusCard({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const account = appRef.current?.getAccount();
  const server = appRef.current?.server;

  const voiceChannelId = server?.voiceChannelId;
  const voiceChannel =
    voiceChannelId && server?.meta
      ? findChannel(server.meta.channels, voiceChannelId)
      : undefined;

  if (!account) return null;

  return (
    <div className="absolute -left-16 right-16 bottom-0 w-[calc(100%+4rem-2rem)] z-10 ml-4 mb-4 @max-[150px]:hidden">
      <Card className="px-3 py-3 h-full w-full flex flex-col gap-1.5">
        {voiceChannel && (
          <div className="text-emerald-500 flex flex-row items-center gap-2 border-b pb-2">
            <Signal className="h-5" />
            <div className="flex flex-col">
              <span>Voice Connected</span>
              <span className="text-xs text-muted-foreground">
                {voiceChannel.name}
              </span>
            </div>
            <Button
              className="ml-auto"
              variant="destructive"
              onClick={() => appRef.current?.leaveVoice()}
            >
              <PhoneOff />
              Disconnect
            </Button>
          </div>
        )}
        <div className="h-full w-full flex flex-row">
          <div className="flex gap-2.5 items-center w-full">
            <Avatar className="h-14 w-auto aspect-square">
              <AvatarImage src={account.meta.avatar} />
              <AvatarFallback>{account.meta.displayName[0]}</AvatarFallback>
            </Avatar>
            <div className="flex flex-col w-full">
              <span
                className="flex items-center gap-1.5 cursor-pointer w-full"
                onClick={() => {
                  navigator.clipboard.writeText(
                    base58.encode(
                      ed.getPublicKey(base58.decode(account.privateKey)),
                    ),
                  );
                }}
              >
                <p>{account.meta.displayName}</p>
                <CopyIcon className="text-muted-foreground size-3" />
              </span>
              <span className="text-muted-foreground">Online</span>
            </div>
          </div>
          <div className="flex items-center text-muted-foreground">
            <Button
              variant={
                appRef.current?.backendConfig.isDeaf ? "destructive" : "ghost"
              }
              className="size-10"
              onClick={() => {
                if (!appRef.current) return;

                appRef.current.backendConfig.isDeaf =
                  !appRef.current.backendConfig.isDeaf;

                updateBackendConfig(appRef.current.backendConfig);

                appRef.current.forceRender();
              }}
            >
              <Headset className="size-5.5" />
            </Button>
            <Button
              variant={
                appRef.current?.backendConfig.isMuted ? "destructive" : "ghost"
              }
              className="size-10"
              onClick={() => {
                if (!appRef.current) return;

                appRef.current.backendConfig.isMuted =
                  !appRef.current.backendConfig.isMuted;

                updateBackendConfig(appRef.current.backendConfig);

                appRef.current.forceRender();
              }}
            >
              {appRef.current?.backendConfig.isMuted ? (
                <MicOff className="size-5.5" />
              ) : (
                <Mic className="size-5.5" />
              )}
            </Button>
            <Button
              variant="ghost"
              className="size-10"
              onClick={() => {
                if (!appRef.current) return;

                appRef.current.isSettingsOpen = !appRef.current?.isSettingsOpen;
                appRef.current.forceRender();
              }}
            >
              <Settings2Icon className="size-5.5" />
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
