import Enclave from "@/app/app";
import ChannelPage from "./channel/ChannelPage";
import { Channel } from "@/lib/types";

export type ChannelPageProps = {
  kind: "channel";
  channel: Channel;
};

export type Page = ChannelPageProps;

export default function Page({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  if (!appRef.current?.page?.kind) return null;

  switch (appRef.current?.page?.kind) {
    case "channel":
      return <ChannelPage appRef={appRef} />;
  }
}
