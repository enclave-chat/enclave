import { Card } from "../ui/card";
import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";
import { CopyIcon, Settings2Icon } from "lucide-react";
import { Button } from "../ui/button";
import Enclave from "@/app/app";
import { base58 } from "@scure/base";
import * as ed from "@noble/ed25519";

export default function AccountCard({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const account = appRef.current?.getAccount();

  if (!account) return null;

  return (
    <div className="absolute -left-16 right-16 bottom-0 h-20 w-[calc(100%+4rem-2rem)] z-10 ml-4 mb-4 @max-[150px]:hidden">
      <Card className="px-3 py-3 h-full w-full flex flex-row">
        <div className="flex gap-2.5 items-center w-full">
          <Avatar className="h-full w-auto aspect-square">
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
      </Card>
    </div>
  );
}
