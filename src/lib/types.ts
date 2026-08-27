export interface ClientMeta {
  displayName: string;
  avatar?: string;
}

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
  is_edited: boolean;
}

export type Channel = { id: string; name: string } & ChannelKind;

export type ChannelKind =
  | { kind: "category"; channels: Channel[] }
  | { kind: "voice"; max_users: number }
  | { kind: "text" };
