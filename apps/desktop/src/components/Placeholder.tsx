export function Placeholder({ title, description }: { title: string; description: string }) {
  return (
    <section className="p-6">
      <h1 className="text-2xl font-semibold">{title}</h1>
      <p className="mt-2 max-w-prose text-muted">{description}</p>
    </section>
  );
}
