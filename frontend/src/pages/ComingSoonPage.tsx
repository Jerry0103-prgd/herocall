type ComingSoonPageProps = {
  title: string;
};

export function ComingSoonPage({ title }: ComingSoonPageProps) {
  return (
    <section className="page coming-soon" aria-labelledby="coming-soon-title">
      <p className="eyebrow">AStock AI Workbench</p>
      <h1 id="coming-soon-title">{title}</h1>
      <p>Coming Soon</p>
    </section>
  );
}
