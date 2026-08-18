export type ViewProps<C> = C extends React.ComponentType<infer P> ? P : never;

export default function SelectView<
  T extends Record<string, React.ComponentType<any>>,
  K extends keyof T,
>({
  config,
  views,
}: {
  config: { currentView: K; viewProps: ViewProps<T[K]> };
  views: T;
}) {
  const ActiveView = views[config.currentView];

  return <ActiveView {...config.viewProps} />;
}
