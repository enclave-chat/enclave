import Enclave from "@/app/app";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  ArrowUpRight,
  BookOpen,
  Fingerprint,
  KeyRound,
  LockKeyhole,
  Network,
  Server,
  ShieldCheck,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";

type Resource = {
  title: string;
  description: string;
  url: string;
  icon: React.ElementType;
};

const APP_RESOURCES: Resource[] = [
  {
    title: "Self-hosted servers",
    description:
      "Enclave connects to servers you or your community control. There is no central company operating the network — your messages live where you choose.",
    url: "https://github.com/tauri-apps/tauri",
    icon: Server,
  },
  {
    title: "Keypair identity",
    description:
      "Your identity is an Ed25519 keypair generated locally. There is no email or password — no third party can reset your account or impersonate you.",
    url: "https://github.com/paulmillr/noble-ed25519",
    icon: Fingerprint,
  },
  {
    title: "Signed, verifiable messages",
    description:
      "Every message carries an Ed25519 signature over its content and timestamp. Anything you receive can be verified to genuinely come from its author.",
    url: "https://github.com/paulmillr/noble-ed25519",
    icon: KeyRound,
  },
  {
    title: "Encrypted realtime transport",
    description:
      "Client and server exchange secrets over an encrypted WebSocket channel, keeping channel traffic and voice metadata away from eavesdroppers.",
    url: "https://github.com/paulmillr/noble-ciphers",
    icon: LockKeyhole,
  },
];

const SECURITY_RESOURCES: Resource[] = [
  {
    title: "End-to-end encryption",
    description:
      "Encrypt data before it leaves your device so only intended recipients can read it. Learn how the model works and where it applies.",
    url: "https://ssd.eff.org/module/why-should-i-use-encryption",
    icon: ShieldCheck,
  },
  {
    title: "Public-key cryptography",
    description:
      "Understand how keypairs sign and encrypt, and why your private key should never leave your device or be shared.",
    url: "https://ssd.eff.org/module/deep-dive-end-end-encryption-how-do-public-key-encryption-systems-work",
    icon: KeyRound,
  },
  {
    title: "Self-hosting & data ownership",
    description:
      "Running your own services means you decide where data is stored and who can access it. Explore guides to getting started.",
    url: "https://www.privacyguides.org/en/self-hosting/",
    icon: Server,
  },
  {
    title: "Transport security",
    description:
      "TLS and WebSocket handshakes protect data in transit. Learn how certificates and secure connections keep traffic safe.",
    url: "https://www.cloudflare.com/learning/ssl/why-is-http-not-secure/",
    icon: Network,
  },
  {
    title: "Threat modeling",
    description:
      "Think about who you are protecting against and what you are protecting. Privacy starts with knowing your threat model.",
    url: "https://www.privacyguides.org/en/basics/threat-modeling/",
    icon: BookOpen,
  },
  {
    title: "Password hygiene & key management",
    description:
      "Good habits for protecting accounts and credentials — and why your cryptographic keys deserve even more care.",
    url: "https://ssd.eff.org/en/module/creating-strong-passwords",
    icon: LockKeyhole,
  },
  {
    title: "Tauri security",
    description:
      "Enclave is built on Tauri, a native desktop shell designed with a minimal attack surface. Review its security guidance.",
    url: "https://v2.tauri.app/security/",
    icon: ShieldCheck,
  },
  {
    title: "Encryption libraries",
    description:
      "Enclave uses audited, dependency-light cryptographic primitives from the noble libraries. Read their documentation.",
    url: "https://github.com/paulmillr/noble-hashes",
    icon: KeyRound,
  },
];

const LEARNING_RESOURCES: Resource[] = [
  {
    title: "Surveillance Self-Defense",
    description:
      "The Electronic Frontier Foundation's free how-to guides for safer online communication.",
    url: "https://ssd.eff.org",
    icon: BookOpen,
  },
  {
    title: "Privacy Guides",
    description:
      "A community hub with practical recommendations for privacy tools and self-hosting.",
    url: "https://www.privacyguides.org",
    icon: ShieldCheck,
  },
  {
    title: "Let's Encrypt",
    description:
      "Free, automated TLS certificates so anyone can secure their own server with HTTPS.",
    url: "https://letsencrypt.org",
    icon: Network,
  },
  {
    title: "The Tor Project",
    description:
      "Research and tools for anonymous, censorship-resistant communication.",
    url: "https://www.torproject.org",
    icon: KeyRound,
  },
];

function ResourceCard({ resource }: { resource: Resource }) {
  const Icon = resource.icon;

  return (
    <Card className="flex flex-col">
      <CardHeader className="flex-1">
        <div className="mb-2 flex h-9 w-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon className="h-4.5 w-4.5" />
        </div>
        <CardTitle className="flex items-center gap-1.5">
          {resource.title}
        </CardTitle>
        <CardDescription>{resource.description}</CardDescription>
      </CardHeader>
      <CardContent>
        <Button
          variant="outline"
          size="sm"
          className="w-full"
          onClick={() => openUrl(resource.url)}
        >
          Learn more
          <ArrowUpRight data-icon="inline-end" />
        </Button>
      </CardContent>
    </Card>
  );
}

function ResourceSection({
  title,
  subtitle,
  resources,
}: {
  title: string;
  subtitle: string;
  resources: Resource[];
}) {
  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold">{title}</h2>
        <p className="text-sm text-muted-foreground">{subtitle}</p>
      </div>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {resources.map((resource) => (
          <ResourceCard key={resource.title} resource={resource} />
        ))}
      </div>
    </section>
  );
}

export default function MainPage(_props: {
  appRef: React.RefObject<Enclave | null>;
}) {
  return (
    <div className="h-screen w-full overflow-y-auto">
      <div className="mx-auto w-full max-w-6xl px-6 py-10">
        <header className="mb-10 flex flex-col items-start gap-4">
          <h1 className="text-3xl font-semibold leading-tight">
            Welcome to Enclave
          </h1>
          <p className="max-w-2xl text-sm text-muted-foreground">
            Enclave is a privacy-focused messenger and voice client built by{" "}
            <span className="font-medium text-foreground">ORUS</span> using
            self-hosted servers, local keypair identities, and cryptographic
            signatures. Nothing here requires an account with a central company
            — you own your identity and your data.
          </p>
          <p className="max-w-2xl text-sm text-muted-foreground">
            Select a server on the left and choose a channel to start chatting.
            Below you'll find resources about how Enclave works, plus guides on
            protecting your privacy and security.
          </p>
        </header>

        <div className="space-y-10">
          <ResourceSection
            title="About this app"
            subtitle="How Enclave keeps your conversations private."
            resources={APP_RESOURCES}
          />

          <ResourceSection
            title="Privacy & security fundamentals"
            subtitle="The concepts and guides behind trustworthy communication."
            resources={SECURITY_RESOURCES}
          />

          <ResourceSection
            title="Learn more"
            subtitle="Organizations and projects worth following."
            resources={LEARNING_RESOURCES}
          />
        </div>
      </div>
    </div>
  );
}
