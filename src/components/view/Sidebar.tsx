import Enclave from "@/app/app";
import { Channel, ChannelKind } from "@/app/protocol";
import { ChevronDown, ChevronUp, HashIcon, Settings2Icon } from "lucide-react";
import { useState } from "react";
import { Card } from "../ui/card";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";
import { Button } from "../ui/button";

export function ChannelIcon({ kind }: { kind: ChannelKind["kind"] }) {
  switch (kind) {
    case "text":
      return <HashIcon className="p-0.5" />;
    default:
      return null;
  }
}

export function RenderCategory({ channel }: { channel: Channel }) {
  if (channel.kind !== "category") return null;

  const [open, setOpen] = useState(true);

  return (
    <div>
      <div
        className="w-full px-2 py-1 rounded-md select-none cursor-default text-sm text-foreground/70 flex items-center"
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
          <RenderChannels channels={channel.channels} />
        </div>
      )}
    </div>
  );
}

export function RenderChannels({ channels }: { channels: Channel[] }) {
  return channels.map((channel) =>
    channel.kind === "category" ? (
      <RenderCategory channel={channel} />
    ) : (
      <div className="w-full hover:bg-accent px-2.5 py-1.5 rounded-md flex gap-2.5 items-center select-none cursor-default text-sm text-muted-foreground">
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
  const account = appRef.current?.getAccount();

  return (
    <div className="h-screen relative w-full @container">
      {appRef.current?.server?.meta && (
        <>
          <header className="px-3 pt-2.5 pb-2.5 text-lg font-semibold border-b border-b-border">
            <h1>{appRef.current.server.meta.name}</h1>
          </header>

          <section className="px-1.5 pt-3.5 w-full flex flex-col gap-1">
            <RenderChannels channels={appRef.current.server.meta.channels} />
          </section>
        </>
      )}

      {account && (
        <div className="absolute -left-16 right-16 bottom-0 h-20 w-[calc(100%+4rem-2rem)] z-10 ml-4 mb-4 @max-[150px]:hidden">
          <Card className="px-3 py-3 h-full w-full flex flex-row">
            <div className="flex gap-2.5 items-center">
              <Avatar className="h-full w-auto aspect-square">
                <AvatarImage src={account.avatar} />
                <AvatarFallback>{account.displayName[0]}</AvatarFallback>
              </Avatar>
              <div className="flex flex-col">
                <span>{account.displayName}</span>
                <span className="text-muted-foreground">Online</span>
              </div>
            </div>
            <div className="ml-auto flex items-center text-muted-foreground">
              <Button variant="ghost" className="size-10">
                <Settings2Icon className="size-5.5" />
              </Button>
            </div>
          </Card>
        </div>
      )}
    </div>
  );
}
