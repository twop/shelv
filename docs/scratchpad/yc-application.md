# Briskmode Labs

## Founders

**How long have the founders known one another and how did you meet? Have any of the founders not met in person?**

We met in the beginning of 2021 working at Next, and kept in touch ever since. Later Mirza invited Simon to work at Pomelo Inc in early 2023, and we have been working together at Pomelo remotely until recently. Over the years we met in person many times.

**Who writes code, or does other technical work on your product? Was any of it done by a non-founder? Please explain.**

Both of us are software engineers, and only the two of us contributed to the codebase.

## Founder Video

- [ ] TODO

## Company

**Company name**

Briskmode Labs

**Describe what your company does in 50 characters or less.**

Productivity tools that inspire joy and wonder

**Company URL, if any**

Unanswered

**Demo Video**

None uploaded

**Please provide a link to the product, if any.**

Unanswered

**If login credentials are required for the link above, enter them here.**

Unanswered

**What is your company going to make? Please describe your product and what it does or will do.**

Shelv is a blend of note-taking apps and computational notebooks.

- Shelv takes the stance that plain-text is king, and with full markdown support, text dictates formatting.
- Rich structural editing akin to Notion (checkboxes, lists etc)
- Extensive keyboard shortcuts, including global shortcut to easily access and hide Shelv without disrupting your focus
- Notes come to life with codeblocks that execute inline (similar to other notebooks), we currently support `javascript`, but `shell`, `sql`, `python` are on the way.
- Hackable and extendable, settings is just another markdown note, hence you can enhance and build custom workflows (like parameterized snippets), or introduce new code blocks type (like running `rust`).
- AI (LLM) can be just another code block (`llm`) that naturally can be context aware inside the note

We believe that Shelv serves an important use case: not all notes are meant to be eternal. We've all had the experience of littering our knowledge bases with temporary notes we'll never look at again. Shelv is the perfect playground for expressing your creativity without the friction. With computation capabilities, you can play with code and data without ever leaving Shelv.

Shelv is an awesome tool for individual productivity, but we believe that effect will compound for teams, for example: multiplayer editing powered by CRDT or exploring data with DuckDB together. What is more fun than playing together?

**Where do you live now, and where would the company be based after YC?**

(Mirza) Reno, NV, USA / (Simon) Alexandria, VA, USA / (Company HQ TBD)

**Explain your decision regarding location.**

The two of us have been working remotely together in some form for over 2 years now. We’re incredibly passionate on how to make collaboration excel in that environment. We also understand the immense value of in-person collaboration, and we intend to continue to spend a significant amount of time working together in-person.

## Progress

**How far along are you?**

Shelv is a fully functioning standalone note-taking app that we’re currently alpha testing on TestFlight with friends & family. We’re planning on releasing to a wider audience soon. Currently, both of us have been dependent on Shelv daily for tasks (including this very application!)
Current feature set:

- Full markdown support text editor
- Note computations through code blocks
- Hackable: Extendable through an API and fully customizable

Coming up next (based on early feedback and what we crave ourselves):

- Sync between devices
- Multi-person collaboration
- Web and mobile versions
- More code blocks: `sql`, `sh`, `llm` etc

All of the above future features represent a significant milestone that depends on a backend for Shelv. We cut those features from the initial release so we can ship faster and gather feedback. But so far, early feedback has shown this seems to be a sticky product.

**How long have each of you been working on this? How much of that has been full-time? Please explain.**

Both of us have been working on Shelv as a side project for over a year now. There have been periods where one of us has been between jobs and has worked on it full-time, but for the most part it has been a side-project while working day jobs.

Our original thinking was to launch Shelv on the Apple App Store and slowly grow revenue (as an indie project), but as we continued development, our vision kept expanding, and we realized a much bigger vision and potential impact this project could have.

**What tech stack are you using, or planning to use, to build this product?**

Our entire stack will be using Rust, which we picked for the particular advantages of the language for this project and our proficiency working with Rust

- Performance and reliability
- Automerge (CRDT)
- Axum on the server
- Rust GUI toolkit (www.egui.rs)
- Building a strong engineering foundation → both to scale the team (we hired Rust engineers to great success in the past) and the product (have experience shipping Rust in production)

**Are people using your product?**

Yes

**How many active users or customers do you have? How many are paying? Who is paying you the most, and how much do they pay you?**

We’re currently testing a pre-alpha version on TestFlight that’s currently being tested by friends and family (<10).

**Anything else you would like us to know regarding your revenue or growth rate?**

Unanswered

**If you are applying with the same idea as a previous batch, did anything change? If you applied with a different idea, why did you pivot and what did you learn from the last idea?**

Unanswered

**If you have already participated or committed to participate in an incubator, "accelerator" or "pre-accelerator" program, please tell us about it.**

Unanswered

## Idea

**Why did you pick this idea to work on? Do you have domain expertise in this area? How do you know people need what you're making?**

Both of us are developers that love optimizing our workflows, and what we noticed is the tooling gap in our personal and work lives: a temporary place to store rough ideas or play with code (often one-off scripts). It feels wrong to create a NodeJS project or a page in Notion for that, we see over and over again, people using tools like Sublime/Notepad++ for transient plain text editing.

Shelv is the tool optimized just for that. In a way, we have a cheat code: scratch our own itch, we are building a tool for ourselves, adjacent, our social circle is the target audience for Shelv.

**Who are your competitors? What do you understand about your business that they don't?**

- Notion

  - Cloud-based, broader audience / general purpose
  - Promotes knowledge base philosophy, hence not conducive for the sense of “play”
  - No scripting capabilities

- Obsidian

  - Personal knowledge base, thus feels heavy weight
  - Some scripting capabilities are via plugins, but not first class
  - Feels sluggish (especially with more plugins), built with web technologies
  - No scratch notes experience (quick notes), at least yet.

- Apple Notes

  - Big advantage of being baked in into the OS
  - Targeting a very broad audience, hence opportunity to niche out.
  - No Windows, Linux and web support.

- Jupyter notebooks (and other notebooks)
  - It is a computational canvas first, not notes/prose.
  - Oriented towards professional use
  - no first class support for collaboration (depending on the app)
  - not local-first, which is an opportunity to differentiate

Those tools are 1:1 competitors in terms of features, but people already use them, and they have overlap, hence we compete for time and headspace. We think that Shelv can have a unique feeling of “playfulness” and be a place for ephemeral thoughts, while combining some features from all of them at the same time.

**How do or will you make money? How much could you make?**
_We realize you can't know precisely, but give your best estimate._

We take inspiration from Warp, Raycast and Obsidian, both in terms of ethos + polish and business model:

- amazing for personal productivity
- compound effect collaborating on a team

Hence, "Personal - free tier", "Personal - pro tier", "Team tier" seem like a good place to start.

Here are some rough numbers:

- Personal: pro -> $5 - $8 a month
- Team: $20-$30 a month/seat

Note that AI features can be separate or bundled, depending on the price/token and usage. For example we can have separate billing for LLM add-ons.

**Which category best applies to your company?**

Other

**If you had any other ideas you considered applying with, please list them. One may be something we've been waiting for. Often when we fund people it's to do something they list here and not in the main application.**

Maybe the local AI version of Shelv that’s all about indexing all of your notes and other local content. Imagine asking the AI assistant “What books did I read last year”. Potentially, local fine-tuning on your data (alongside vector search)

## Equity

**Have you formed ANY legal entity yet?**
_This may be in the US, in your home country, or in another country._

No

**If you have not formed the company yet, describe the planned equity ownership breakdown among the founders, employees and any other proposed stockholders. If there are multiple founders, be sure to give the proposed equity ownership of each founder and founder title (e.g. CEO). (This question is as much for you as us.)**

Simon - CEO/Product - 51%
Mirza - CTO/Eng - 49%

Basically as close to 50/50 as possible, taking into account other equity allocations

**Have you taken any investment yet?**

No

**Are you currently fundraising?**

No

## Curious

**What convinced you to apply to Y Combinator? Did someone encourage you to apply? Have you been to any YC events?**

While working on Shelv we were expanding the vision, and now we got to the point when taking an investment and go full-time is what we both want

We have a mutual friend who was a part of a couple of YC applications, and when he saw Shelv he strongly encouraged us to apply.

**How did you hear about Y Combinator?**

We knew about YC for quite a while, who doesn't? ^\_^
