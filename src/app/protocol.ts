export interface ClientMeta {}

export interface ServerMeta {
  name: string;
  description: string;
  channels: Channel[];
}

export type ClientMethod =
  | {
      method: "Initialized";
      public_key: string;
      signature: string;
      timestamp: number;
      hostname: string;
    }
  | {
      method: "Error";
      error: string;
    };

export type ServerMethod =
  | {
      method: "Initialize";
      public_key: string;
      signature: string;
      timestamp: number;
      hostname: string;
    }
  | {
      method: "Meta";
    }
  | {
      method: "Error";
      error: string;
    };

export type Channel = { id: string; name: string } & ChannelKind;

export type ChannelKind =
  { kind: "text" } | { kind: "category"; channels: Channel[] };
