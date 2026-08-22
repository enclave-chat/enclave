import Enclave from "@/app/app";
import { ChannelPageProps } from "@/components/page/PageView";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ClientMeta, StoredMessage } from "@/lib/types";
import { base58 } from "@scure/base";
import { useEffect, useRef, useState } from "react";
import * as ed from "@noble/ed25519";
import { CircleXIcon } from "lucide-react";

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

  const messages = appRef.current?.server?.messages[channel.id];

  const sendMessage = () => {
    appRef.current?.sendMessage(currentMessage, channel.id);
    setCurrentMessage("");
  };

  return (
    <div className="flex flex-col h-screen gap-2">
      <header className="px-3 pt-3 pb-3 text-sm text-muted-foreground border-b border-b-border">
        <h2>{channel.name}</h2>
      </header>

      <div className="h-full flex flex-col-reverse gap-2.5 px-3 pt-4 overflow-y-scroll">
        {messages &&
          Object.entries(messages)
            .sort((a, b) => b[1].timestamp - a[1].timestamp)
            .map(([_, message]) => (
              <TextMessage key={message.id} appRef={appRef} message={message} />
            ))}
        <div ref={intersectionRef} />
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

  const author = appRef.current?.server?.users[message.author];

  const [verified, setVerified] = useState(true);

  useEffect(() => {
    const serverPubKey =
      appRef.current?.server?.serverPublicKey &&
      base58.encode(appRef.current?.server?.serverPublicKey);

    if (!serverPubKey) return;

    const authorPubKey = base58.decode(message.author);

    setVerified(
      ed.verify(
        base58.decode(message.signature),
        new TextEncoder().encode(
          `${message.timestamp}@${serverPubKey}@${message.content}`,
        ),
        authorPubKey,
      ),
    );
  }, []);

  return (
    <div className="rounded-lg flex gap-3 px-3 py-3 hover:bg-muted/40">
      <Avatar className="h-10 w-10">
        <AvatarImage src={author?.avatar} />

        <AvatarFallback>
          {author?.displayName.slice(0, 1).toUpperCase() ||
            message.author.slice(0, 2).toUpperCase()}
        </AvatarFallback>
      </Avatar>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium flex items-center gap-1">
            {!verified && (
              <CircleXIcon className="h-3.5 w-3.5 text-destructive" />
            )}
            {author?.displayName || message.author.slice(0, 8)}
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
