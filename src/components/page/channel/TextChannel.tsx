import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { StoredMessage } from "@/lib/types";
import { useEffect, useRef, useState } from "react";

export default function TextChannel({
  appRef,
}: {
  appRef: React.RefObject<Enclave<ChannelPageProps> | null>;
}) {
  const [currentMessage, setCurrentMessage] = useState("");
  const channel = appRef.current?.page?.channel;

  const intersectionRef = useRef<HTMLDivElement | null>(null);
  const chunkRef = useRef(0);

  useEffect(() => {
    if (!intersectionRef.current) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          console.log("Element is in view!");
          if (!channel) return;

          appRef.current?.server?.websocket?.send({
            method: "GetMessages",
            channel_id: channel.id,
            chunk: chunkRef.current,
          });

          appRef.current?.forceRender();
          chunkRef.current += 1;
        }
      },
      {
        threshold: 0.5,
      },
    );

    observer.observe(intersectionRef.current);

    return () => observer.disconnect();
  }, []);

  if (!channel) return null;

  const sendMessage = () => {
    appRef.current?.sendMessage(currentMessage, channel.id);
    setCurrentMessage("");
  };

  return (
    <div className="flex flex-col h-screen">
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border">
        <h2>{channel.name}</h2>
      </header>

      <div className="h-full flex flex-col gap-2.5 px-3 pt-4 overflow-y-scroll">
        <div ref={intersectionRef} />
        {appRef.current?.server?.messages[channel.id] &&
          Object.entries(appRef.current?.server?.messages[channel.id]).map(
            ([_, message]) => (
              <TextMessage key={message.id} appRef={appRef} message={message} />
            ),
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
      <Avatar className="h-10 w-10">
        <AvatarFallback>
          {message.author.slice(0, 2).toUpperCase()}
        </AvatarFallback>
      </Avatar>

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
