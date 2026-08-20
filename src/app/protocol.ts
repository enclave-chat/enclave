export interface ClientMeta {}

export interface ServerMeta {
  name: string;
  description: string;
  channels: Channel[];
}

export interface MessageData {
  content: string;
  timestamp: number;
  signature: string;
}

export interface StoredMessage extends MessageData {
  id: string;
  author: string;
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
      method: "SendMessage";
      channel_id: string;
      data: MessageData;
    }
  | {
      method: "Error";
      error: string;
    };

export type Channel = { id: string; name: string } & ChannelKind;

export type ChannelKind =
  { kind: "text" } | { kind: "category"; channels: Channel[] };
