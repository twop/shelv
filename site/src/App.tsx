import { Fragment, h } from "preact";
import * as Preact from "preact";

import { colors } from "./theme";
import { ShelvLogo } from "./Logo";
// import { codeSnippetHtml } from "./code-snippet";
const Content: Preact.FunctionalComponent = ({ children }) => (
  <div className="mx-auto px-4 sm:px-6 max-w-6xl">{children}</div>
);

const Theme: Preact.FunctionalComponent<{ color: "light" | "dark" }> = ({
  children,
  color,
}) => (
  <div
    className={`relative text-base text-nord-text ${
      color === "light" ? "bg-nord-bg" : "bg-nord-bg-dark"
    }`}
  >
    {children}
  </div>
);

const IMG_W = 1180;
const IMG_H = 1128;

const UP_WAVE_PATH =
  "M0,128L120,144C240,160,480,192,720,208C960,224,1200,224,1320,224L1440,224L1440,320L1320,320C1200,320,960,320,720,320C480,320,240,320,120,320L0,320Z";
const DOWN_WAVE_PATH =
  "M0,224L80,186.7C160,149,320,75,480,53.3C640,32,800,64,960,85.3C1120,107,1280,117,1360,122.7L1440,128L1440,320L1360,320C1280,320,1120,320,960,320C800,320,640,320,480,320C320,320,160,320,80,320L0,320Z";

const Wave = ({
  path,
  topColor,
}: {
  path: string;
  topColor: "bg-nord-bg" | "bg-nord-bg-dark";
}) => (
  <div className={topColor}>
    <svg viewBox="0 0 1440 160" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <filter id="shadow">
          <feDropShadow dx="0" dy="-20" stdDeviation="5" />
        </filter>
      </defs>
      <g transform="scale(1, 0.5)">
        <path
          fill={colors[topColor === "bg-nord-bg" ? "nord-bg-dark" : "nord-bg"]}
          d={path}
        ></path>
      </g>
    </svg>
  </div>
);

const Block: Preact.FunctionComponent<{
  left: Preact.VNode;
  right: Preact.VNode;
  main?: "left" | "right";
}> = ({ left, right, main = "left" }) => {
  // <div className="max-w-2xl">{left}</div>
  return (
    <div className="relative">
      <div
        className={`flex ${
          main === "right" ? "flex-col-reverse" : "flex-col"
        } lg:flex-none lg:grid lg:grid-flow-row-dense lg:grid-cols-2 lg:gap-10 lg:items-center`}
      >
        <div className="lg:pr-4">{left}</div>
        <div className="lg:pl-4">{right}</div>
      </div>
    </div>
  );
};

const BlockHeader: Preact.FunctionComponent = ({ children }) => (
  <h4 className="text-2xl mb-4 leading-8 font-semibold sm:text-3xl sm:leading-9 text-nord-h2">
    {children}
  </h4>
);

const BlockText: Preact.FunctionComponent = ({ children }) => (
  <p className="text-lg leading-7">{children}</p>
);

const PageHeader = () => (
  <div className="flex justify-between items-center py-6">
    <div className="inline-flex items-center space-x-2 leading-6 font-medium transition ease-in-out duration-150">
      {/* <img
        src="images/shelv-logo-svg.svg"
        alt="Shelv app logo"
        height="68"
        width="198"
      /> */}
      <ShelvLogo></ShelvLogo>
    </div>

    <div className="flex gap-x-12">
      {[
        { name: "FAQ", href: "#" },
        { name: "License", href: "#" },
      ].map((item) => (
        <a
          key={item.name}
          href={item.href}
          className="text-sm leading-6 text-nord-text-subtle"
        >
          {item.name}
        </a>
      ))}
    </div>
    {/* <div className="flex flex-shrink flex-row">
      <SvgIconLink
        size="large"
        linkTo="https://github.com/twop"
        path={githubSvgPath}
      />
      <div className="w-6"></div>
      <SvgIconLink
        size="large"
        linkTo="https://twitter.com/shelvdotapp"
        path={twitterSvgPath}
      />
    </div> */}
  </div>
);

// ## The ultimate playground for your ideas
// Where you can capture and organize them in a Markdown wonderland. It’s the perfect tool for those who need a fun and efficient way to manage their thoughts. Whether you’re planning a trip, organizing your daily tasks, or brainstorming your next big idea.
const SloganAndMacStoreLink = () => (
  <Fragment>
    <div className="text-center lg:text-left">
      <div className="">
        <h2 className="text-4xl leading-10 font-semibold sm:text-5xl sm:leading-none lg:text-5xl text-nord-h1">
          The ultimate playground for your ideas
        </h2>
        <p className="mt-4 max-w-md mx-auto text-lg sm:text-xl md:mt-5 md:max-w-3xl">
          Where you can capture and organize them in a Markdown wonderland. It's
          the perfect tool for those who need a fun and efficient way to manage
          their thoughts. Whether you're planning a trip, organizing your daily
          tasks, or brainstorming your next big idea.
        </p>
      </div>
    </div>
    <div className="mt-8 sm:max-w-lg sm:mx-auto text-center sm:text-center lg:text-left lg:mx-0">
      <MacStoreLink />
    </div>
  </Fragment>
);

const MacStoreLink: Preact.FunctionComponent<{}> = () => {
  return (
    <a href="">
      <img
        src="/images/mac-app-store-badge.svg"
        alt="Download Shelv on the Mac App Store"
        class="home-app-store-buttons-mac"
        // width="auto"
        height="64"
      />
    </a>
  );
};

const Img: Preact.FunctionComponent<{
  src: string;
  alt: string;
  width: number;
  height: number;
  extraStyle?: string;
  eager?: boolean;
}> = ({ src, alt, extraStyle, width, height, eager }) => (
  <div className="py-6 lg:py-0 w-full h-full flex justify-center">
    <img
      // className="transition-opacity duration-1000 delay-200 fade-in"
      width={width}
      height={height}
      loading={eager ? "eager" : "lazy"}
      alt={alt}
      src={`images/${src}.png`}
    ></img>
  </div>
);

const Space: Preact.FunctionComponent<{
  sm?: boolean;
  lg?: boolean;
  md?: boolean;
  extraOnLarge?: boolean;
}> = ({ sm, lg, md, extraOnLarge }) => (
  <div
    className={`w-full ${
      sm
        ? "h-4 sm:h-8"
        : md
        ? "h-8 sm:h-12"
        : lg
        ? "h-12 sm:h-16"
        : "h-8 sm:h-12"
    } ${extraOnLarge ? "lg:my-6" : ""}`}
  ></div>
);

export const App = () => (
  <Fragment>
    <Theme color="dark">
      <Content>
        <PageHeader />
      </Content>
      <Content>
        <Block
          left={<SloganAndMacStoreLink />}
          right={
            <Img
              width={IMG_W}
              height={IMG_H}
              src="screenshot-welcome"
              eager
              alt="app screenshot with welcome note"
            />
          }
        />
      </Content>
    </Theme>
    <Wave path={UP_WAVE_PATH} topColor="bg-nord-bg-dark" />
    <Theme color="light">
      <Content>
        <Block
          left={
            <Fragment>
              <BlockHeader>
                {/* <span className="text-nord-text-primary">Step 1: </span>{" "} */}
                Markdown Native
              </BlockHeader>
              <BlockText>
                Shelv is built on markdown, which means you can quickly format
                your ideas in an expressive way that is open and portable to
                where ever they need to go.
              </BlockText>
            </Fragment>
          }
          right={
            <Img
              width={IMG_W}
              height={IMG_H}
              src="screenshot-markdown"
              alt="app screenshot with markdown features"
            />
          }
        />

        <Space extraOnLarge />
        <Block
          left={
            <Img
              width={IMG_W}
              height={IMG_H}
              src="screenshot-shortcuts"
              alt="app screenshot with shortcuts"
            />
          }
          right={
            <Fragment>
              <BlockHeader>
                {/* <span className="text-nord-text-primary">Step 2: </span> */}
                Keyboard shortcuts
              </BlockHeader>
              <BlockText>
                Show/Hide Shelv with a system wide shortcut, so it is there when
                you need it.
              </BlockText>
              <Space sm />
              <BlockText>
                Annotation shortcuts for <b>Bold</b>, <i>Italic</i>, Headings
                and <code>Code blocks</code>
              </BlockText>
            </Fragment>
          }
          main="right"
        />
        <Space sm />
      </Content>
    </Theme>
    <Wave path={DOWN_WAVE_PATH} topColor="bg-nord-bg" />
    <Theme color="dark">
      <Content>
        <div className="w-full px-4">
          <div className="border-solid border-t-1 w-full border-nord-line-break mt-8 mb-6" />
          <div>
            <p className="mt-3 text-m leading-7">
              Done with <Heart /> by Simon Korzunov (
              <SvgIconLink
                linkTo="https://github.com/twop"
                path={githubSvgPath}
              />{" "}
              <SvgIconLink
                linkTo="https://twitter.com/twopSK"
                path={twitterSvgPath}
              />
              )
            </p>
            <p className="mt-3 text-m leading-7">
              Shoot us an email at{" "}
              <Link to="mailto:hi@shelv.app">hi@shelv.app</Link>
            </p>
            <div className="py-3 flex justify-end">
              <p className="text-xs leading-7">
                theme inspired by{" "}
                <Link to="https://www.nordtheme.com/">Nord</Link>
              </p>
            </div>
          </div>
        </div>
      </Content>
    </Theme>
  </Fragment>
);

const Heart = () => (
  <svg
    className="text-nord-red h-4 w-4 inline"
    fill="none"
    viewBox="0 0 24 24"
    stroke="currentColor"
  >
    <path
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth={2}
      d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"
    />
  </svg>
);
const Link: Preact.FunctionComponent<{ to: string }> = ({ children, to }) => (
  <a className="text-nord-text-primary hover:underline" href={to}>
    {children}
  </a>
);

const SvgIconLink: Preact.FunctionComponent<{
  linkTo: string;
  path: Preact.VNode;
  size?: "small" | "large";
}> = ({ path, linkTo, size = "small" }) => (
  <a href={linkTo}>
    <svg
      className={
        "inline fill-current text-nord-text-primary hover:text-nord-bg-btn-hovered" +
        (size === "small" ? "h-4 w-4" : "h-8 w-8")
      }
      viewBox="0 0 24 24"
      enable-background="new 0 0 24 24"
    >
      {path}
    </svg>
  </a>
);

const twitterSvgPath = (
  <path
    d="M17.316,6.246c0.008,0.162,0.011,0.326,0.011,0.488c0,4.99-3.797,10.742-10.74,10.742c-2.133,0-4.116-0.625-5.787-1.697
	c0.296,0.035,0.596,0.053,0.9,0.053c1.77,0,3.397-0.604,4.688-1.615c-1.651-0.031-3.046-1.121-3.526-2.621
	c0.23,0.043,0.467,0.066,0.71,0.066c0.345,0,0.679-0.045,0.995-0.131c-1.727-0.348-3.028-1.873-3.028-3.703c0-0.016,0-0.031,0-0.047
	c0.509,0.283,1.092,0.453,1.71,0.473c-1.013-0.678-1.68-1.832-1.68-3.143c0-0.691,0.186-1.34,0.512-1.898
	C3.942,5.498,6.725,7,9.862,7.158C9.798,6.881,9.765,6.594,9.765,6.297c0-2.084,1.689-3.773,3.774-3.773
	c1.086,0,2.067,0.457,2.756,1.191c0.859-0.17,1.667-0.484,2.397-0.916c-0.282,0.881-0.881,1.621-1.66,2.088
	c0.764-0.092,1.49-0.293,2.168-0.594C18.694,5.051,18.054,5.715,17.316,6.246z"
  />
);

const githubSvgPath = (
  <path
    d="M13.18,11.309c-0.718,0-1.3,0.807-1.3,1.799c0,0.994,0.582,1.801,1.3,1.801s1.3-0.807,1.3-1.801
  C14.479,12.116,13.898,11.309,13.18,11.309z M17.706,6.626c0.149-0.365,0.155-2.439-0.635-4.426c0,0-1.811,0.199-4.551,2.08
  c-0.575-0.16-1.548-0.238-2.519-0.238c-0.973,0-1.945,0.078-2.52,0.238C4.74,2.399,2.929,2.2,2.929,2.2
  C2.14,4.187,2.148,6.261,2.295,6.626C1.367,7.634,0.8,8.845,0.8,10.497c0,7.186,5.963,7.301,7.467,7.301
  c0.342,0,1.018,0.002,1.734,0.002c0.715,0,1.392-0.002,1.732-0.002c1.506,0,7.467-0.115,7.467-7.301
  C19.2,8.845,18.634,7.634,17.706,6.626z M10.028,16.915H9.972c-3.771,0-6.709-0.449-6.709-4.115c0-0.879,0.31-1.693,1.047-2.369
  c1.227-1.127,3.305-0.531,5.662-0.531c0.01,0,0.02,0,0.029,0c0.01,0,0.018,0,0.027,0c2.357,0,4.436-0.596,5.664,0.531
  c0.735,0.676,1.045,1.49,1.045,2.369C16.737,16.466,13.8,16.915,10.028,16.915z M6.821,11.309c-0.718,0-1.3,0.807-1.3,1.799
  c0,0.994,0.582,1.801,1.3,1.801c0.719,0,1.301-0.807,1.301-1.801C8.122,12.116,7.54,11.309,6.821,11.309z"
  />
);
