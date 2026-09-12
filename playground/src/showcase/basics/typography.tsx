import { Demo, Section } from "../shared"

export function TypographySection() {
  return (
    <Section
      id="typography"
      title="Typography"
      group="installation"
      description="A set of text primitives for consistent typography."
      fileKey="basics"
    >
      {/* ========================================================= */}
      {/* ===================== TYPOGRAPHY ======================== */}
      {/* ========================================================= */}

      <Demo
        title="Typography · Headings"
        description="All heading levels from h1 to h4."
      >
        <div className="flex flex-col gap-3">
          <h1 className="scroll-m-20 text-4xl font-extrabold tracking-tight lg:text-5xl">
            Heading 1
          </h1>
          <h2 className="scroll-m-20 text-3xl font-semibold tracking-tight first:mt-0">
            Heading 2
          </h2>
          <h3 className="scroll-m-20 text-2xl font-semibold tracking-tight">
            Heading 3
          </h3>
          <h4 className="scroll-m-20 text-xl font-semibold tracking-tight">
            Heading 4
          </h4>
        </div>
      </Demo>

      <Demo
        title="Typography · Paragraphs"
        description="Standard paragraph text with lead variant."
      >
        <div className="flex flex-col gap-4">
          <p className="leading-7 [&:not(:first-child)]:mt-6">
            The full legal name means the board shall consist of the President and Members of the Board,
            each of whom shall be appointed by the President of the Enterprise and shall hold office
            during the pleasure of the President. The President shall designate a Chairman.
          </p>
          <p className="leading-7 [&:not(:first-child)]:mt-6">
            A <span className="bg-muted font-semibold">paragraph</span> is a self-contained unit
            of discourse in writing dealing with a particular point or idea. Paragraphs are usually
            an expected part of formal writing, used to organize longer prose.
          </p>
          <p className="leading-7 [&:not(:first-child)]:mt-6">
            The lead paragraph is the opening paragraph of an article or chapter. It is designed to
            give a quick summary of the following content.
          </p>
          <p className="text-lg text-muted-foreground [&:not(:first-child)]:mt-6">
            This is a lead paragraph — larger, muted text used to introduce a section.
          </p>
        </div>
      </Demo>

      <Demo
        title="Typography · Lists"
        description="Unordered, ordered, and nested lists."
      >
        <div className="flex flex-col gap-4">
          <ul className="my-4 ml-6 list-disc [&>li]:mt-2">
            <li>First list item</li>
            <li>Second list item</li>
            <li>Third list item</li>
          </ul>
          <ol className="my-4 ml-6 list-decimal [&>li]:mt-2">
            <li>First ordered item</li>
            <li>Second ordered item</li>
            <li>Third ordered item</li>
          </ol>
          <ul className="my-4 ml-6 list-disc [&>li]:mt-2 [&>ul]:mt-2 [&>ul>li]:text-muted-foreground">
            <li>
              List item with nested sub-list
              <ul>
                <li>Sub-item one</li>
                <li>Sub-item two</li>
              </ul>
            </li>
            <li>
              Another top-level item
              <ul>
                <li>Sub-item</li>
              </ul>
            </li>
          </ul>
        </div>
      </Demo>

      <Demo
        title="Typography · Blockquote"
        description="Blockquotes for citing content."
      >
        <blockquote className="mt-6 border-l-2 border-border pl-6 italic text-muted-foreground [&>p]:leading-7">
          <p>
            "The only way to do great work is to love what you do. If you haven't found it yet,
            keep looking. Don't settle."
          </p>
          <cite className="mt-4 block not-italic text-sm font-medium text-foreground">
            — Steve Jobs
          </cite>
        </blockquote>
      </Demo>

      <Demo
        title="Typography · Inline Code"
        description="Inline code and code blocks."
      >
        <div className="flex flex-col gap-3">
          <p className="leading-7">
            The <code className="bg-muted rounded-sm px-1 py-0.5 font-mono text-sm">const</code>{" "}
            keyword is used to declare a constant in JavaScript. The{" "}
            <code className="bg-muted rounded-sm px-1 py-0.5 font-mono text-sm">function</code>{" "}
            keyword declares a function.
          </p>
          <pre className="mt-4 overflow-auto rounded-lg border bg-muted p-4 font-mono text-sm">
            <code>{`function greet(name) {
  return \`Hello, \${name}!\`;
}`}</code>
          </pre>
        </div>
      </Demo>

      <Demo
        title="Typography · Small & Muted"
        description="Small and muted text variants."
      >
        <div className="flex flex-col gap-3">
          <p className="text-sm font-medium">Medium small text</p>
          <p className="text-sm text-muted-foreground">
            Muted text is used for secondary information, captions, and helper text.
          </p>
          <p className="text-xs text-muted-foreground">
            Extra small muted text — for labels, timestamps, and metadata.
          </p>
          <p className="text-xs">
            Extra small text — the smallest readable size.
          </p>
        </div>
      </Demo>

      <Demo
        title="Typography · Large"
        description="Large text variant for emphasis."
      >
        <p className="text-lg font-semibold">
          A large body of text, used for introductory paragraphs or callouts.
        </p>
      </Demo>

      <Demo
        title="Typography · Links"
        description="Inline anchor links with hover state."
      >
        <p className="leading-7">
          Visit the{" "}
          <a
            href="#"
            className="font-medium text-primary underline-offset-4 hover:underline"
          >
            official documentation
          </a>{" "}
          for more information. You can also check the{" "}
          <a
            href="#"
            className="font-medium text-primary underline-offset-4 hover:underline"
          >
            API reference
          </a>
          .
        </p>
      </Demo>
    </Section>
  )
}
