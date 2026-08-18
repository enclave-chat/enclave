import Enclave from "@/app/app";

export default function ServerList({
  appRef,
}: {
  appRef: React.RefObject<Enclave | null>;
}) {
  return (
    <aside className="bg-card w-16 h-screen p-2 flex flex-col gap-2.5">
      {appRef.current?.serverList.map((server) => (
        <img
          src={`http://${server.hostname}/icon`}
          className={
            server.publicKey === appRef.current?.server?.hostname
              ? "rounded-lg"
              : "rounded-full"
          }
        />
      ))}
    </aside>
  );
}
