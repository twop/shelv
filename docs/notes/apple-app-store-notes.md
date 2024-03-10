---
Date: 2024/Mar/9
---

## Monetization in iOS

- **Ads**
  - Duo lingo
- **Upfront**, e.g $5 for donwload
- **Subsription**
  - Apple Id (as a user)
    - My subs => store tracks it
  - Monthly, Yearly, Country
    - as a developer (developer acc)
  - App redirects to the store to buy a consumable
    - User goes to the app store
  - On focusing back you need to check
    - consumable ID,
  - As a _user_
    - redirected to the app store
    - choose a subsription (out of the list) -> buy
    - go back to the application
  - as a _developer_
    - consumables (via UI on App store dev portal)
    - when user opens up the app check for purchased consumbles
      - there is iOS api to check for consumbles
    - Application redirects to the app store
- **in App purchase**
  - items in games
  - you can buy features in normal application (one password)
  - no expiration dates for "in app" purchase
    - as a consequence: bought vs didn't buy
  - Q: API?
    - user needs to go to the app store to buy "in app"
- **Mixture of all of the above**
  - Duo lingo
    - literally all:
      - Trial 2 weeks (demo)
      - Demo expired? => Ads (funnyly enough themselves)
      - In app (you can buy hearts, even though they are permanent but consumable inside the app)
      - subscription
    - **Duo lingo has a pro trial for free? `how`?**

## Questions

1. Q: License key? Technically possible, but legally interesting
   - before Fortnight and UE, not possible
   - opportunity cost for the App store, thus not possible (at least in the past)
   - related: you cannot buy anything outside of app store if the link is available inside the app
   - It is possible that it will change soon due to EU
   - in short **no**
   - app codes? (native iOS capabilities), e.g. license key
     - works to downlaod paid application for free, but not sure for subscriptions and such
2. Q: Grandfather
   - Pro (in app) -> Subscription (from UX point of view)
   - One password was not able to do that, they had to publish a new app
     - It is possible that converting "In App" -> Subs is against ToS
   - Q: Can I give a consumable to the user?
3. Apple login? Do I need to support it?
   - if I create an account do I need to support apple login
   - A: privacy and information control
     - website with ToS
     - you need to support account deletion
       - how?
       - A: possible even manual deletion will be OK, ideally link withing the App
4. What do I know about the user(as a developer)?
   - can I get email of the user?
     - No by default, only anonynous "user id" bought an item
       - you can get an installation ID and correlate it with purchased items?
         - do I have an ID of a purchase? `TBD`
     - Apple won't give you that
