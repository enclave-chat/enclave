import Enclave from "@/app/app";
import { Channel, ChannelKind } from "@/lib/types";
import { ChevronDown, ChevronUp, HashIcon, Volume2Icon } from "lucide-react";
import { useState } from "react";
import StatusCard from "./StatusCard";
import { cn } from "@/lib/utils";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";

export function ChannelIcon({ kind }: { kind: ChannelKind["kind"] }) {
  switch (kind) {
    case "category":
      return null;
    case "text":
      return <HashIcon className="p-0.5" />;
    case "voice":
      return <Volume2Icon className="p-0.5" />;
  }
}

export function RenderCategory({
  appRef,
  channel,
}: {
  appRef: React.RefObject<Enclave | null>;
  channel: Channel;
}) {
  if (channel.kind !== "category") return null;

  const [open, setOpen] = useState(true);

  return (
    <div>
      <div
        className="w-full px-2 py-1 rounded-md cursor-default text-sm text-foreground/70 flex items-center"
        onClick={() => setOpen((v) => !v)}
      >
        {channel.name}
        {open ? (
          <ChevronDown className="p-1 shrink" />
        ) : (
          <ChevronUp className="p-1 shrink" />
        )}
      </div>
      {open && (
        <div className="pt-0.5 w-full flex flex-col gap-1">
          <RenderChannels appRef={appRef} channels={channel.channels} />
        </div>
      )}
    </div>
  );
}

export function RenderFeatures({
  appRef,
  channel,
}: {
  appRef: React.RefObject<Enclave | null>;
  channel: Channel;
}) {
  switch (channel.kind) {
    case "voice":
      const users = appRef.current?.server?.voiceChatUsers[channel.id];

      return (
        users && (
          <div className="flex flex-col gap-1.5 px-4 pt-0.5">
            {users.map((pubkey) => {
              const user = appRef.current?.server?.users[pubkey];
              if (!user) return null;

              return (
                <div key={pubkey} className="flex flex-row items-center gap-2">
                  <Avatar className="h-7 w-7">
                    <AvatarImage src={user.avatar} />
                    <AvatarFallback>
                      {user?.displayName.slice(0, 1).toUpperCase() ||
                        pubkey.slice(0, 2).toUpperCase()}
                    </AvatarFallback>
                  </Avatar>

                  <span className="text-sm text-muted-foreground">
                    {user.displayName}
                  </span>
                </div>
              );
            })}
          </div>
        )
      );
    default:
      return null;
  }
}

export function RenderChannels({
  appRef,
  channels,
}: {
  appRef: React.RefObject<Enclave | null>;
  channels: Channel[];
}) {
  return channels.map((channel) =>
    channel.kind === "category" ? (
      <RenderCategory key={channel.id} appRef={appRef} channel={channel} />
    ) : (
      <div className="flex flex-col" key={channel.id}>
        <div
          className={cn(
            "w-full hover:bg-accent px-2.5 py-1.5 rounded-md flex gap-2.5 items-center select-none cursor-default text-sm text-muted-foreground",
            channel.id === appRef.current?.page?.channel.id,
          )}
          onClick={() => {
            if (!appRef.current) {
              console.error("AppRef is not initialized yet");
              return;
            }

            appRef.current.page = { kind: "channel", channel };

            appRef.current.forceRender();
          }}
        >
          <ChannelIcon kind={channel.kind} />
          {channel.name}
        </div>
        <RenderFeatures appRef={appRef} channel={channel} />
      </div>
    ),
  );
}

export default function Sidebar({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  return (
    <div className="h-screen relative w-full @container">
      {appRef.current?.server?.meta && (
        <>
          <header className="px-3 pt-2.5 pb-2.5 text-lg font-semibold border-b border-b-border">
            <h1>{appRef.current.server.meta.name}</h1>
          </header>

          <section className="px-1.5 pt-3.5 w-full flex flex-col gap-1">
            <RenderChannels
              appRef={appRef}
              channels={appRef.current.server.meta.channels}
            />
          </section>
        </>
      )}

      <StatusCard appRef={appRef} />
    </div>
  );
}
