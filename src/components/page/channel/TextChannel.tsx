import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StoredMessage } from "@/lib/types";
import { useEffect, useState } from "react";

export default function TextChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const [currentMessage, setCurrentMessage] = useState("");
  const channel = appRef.current?.page?.channel;

  useEffect(() => {
    if (!channel) return;

    appRef.current?.server?.websocket?.send({
      method: "GetMessages",
      channel_id: channel.id,
      chunk: 0,
    });

    appRef.current?.forceRender();
  }, [channel]);

  if (!channel) return null;

  const sendMessage = () => {
    appRef.current?.sendMessage(currentMessage, channel.id);
    setCurrentMessage("");
  };

  return (
    <div className="flex flex-col h-full">
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border">
        <h2>{channel.name}</h2>
      </header>

      <div className="h-full flex flex-col gap-2.5 px-3 pt-4 overflow-y-scroll">
        {appRef.current?.server?.messages[channel.id] &&
          Array.from(appRef.current?.server?.messages[channel.id]).map(
            (message) => <TextMessage appRef={appRef} message={message} />,
          )}
      </div>

      <div className="pb-6 px-4 flex flex-row gap-3">
        <Input
          className="h-10"
          placeholder={`Message ${channel.name}`}
          value={currentMessage}
          onChange={(e) => setCurrentMessage(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              sendMessage();
            }
          }}
        />

        {currentMessage.trim() && <Button onClick={sendMessage}>Send</Button>}
      </div>
    </div>
  );
}

export function TextMessage({
  appRef,
  message,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
  message: StoredMessage;
}) {
  const time = new Date(message.timestamp).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });

  return (
    <div className="rounded-lg flex gap-3 px-3 py-3 hover:bg-muted/40">
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium text-muted-foreground">
        {message.author.slice(0, 2).toUpperCase()}
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="text-sm font-medium">
            {message.author.slice(0, 8)}
          </span>
          <span className="text-xs text-muted-foreground">{time}</span>
        </div>
        <p className="whitespace-pre-wrap break-words text-sm leading-snug">
          {message.content}
        </p>
      </div>
    </div>
  );
}
