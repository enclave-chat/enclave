import Enclave from "@/app/app";
import EnclaveServer from "@/app/server";
import { getHTTPUrl, getWSUrl } from "@/lib/serverList";

export default function ServerList({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  return (
    <aside className="bg-card w-16 h-screen p-2 flex flex-col gap-2.5">
      {appRef.current?.serverList.map((server) => {
        return (
          <img
            src={getHTTPUrl(server, "/icon")}
            className={
              server.publicKey === appRef.current?.server?.hostname
                ? "rounded-lg"
                : "rounded-full"
            }

            onClick={async () => {
              if (!appRef.current) {
                console.error("AppRef is not initialized yet");
                return;
              }

              if (appRef.current.server) {
                appRef.current.server.disconnect();
              }

              appRef.current.server = new EnclaveServer(getWSUrl(server));
            }}
          />
        );
      })}
    </aside>
  );
}
