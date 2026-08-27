import Enclave from "@/app/app";
import { Channel, ChannelKind } from "@/lib/types";
import { ChevronDown, ChevronUp, HashIcon, Volume2Icon } from "lucide-react";
import { useState } from "react";
import AccountCard from "./AccountCard";
import { cn } from "@/lib/utils";

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
      <div
        key={channel.id}
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

      <AccountCard appRef={appRef} />
    </div>
  );
}
