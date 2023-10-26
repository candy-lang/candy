# Contributing

Thank you for your interest in contributing to Candy! 🎉

We use [Discord](https://discord.gg/5Vr4eAJ7gU) for communication.
If you have any questions, please don't hesitate to ask us there :)

## Local Setup

Follow the [README's How to use Candy section](https://github.com/candy-lang/candy#how-to-use-candy) to set up your development environment.

## Selecting an issue

Visit the [GitHub Project](https://github.com/orgs/candy-lang/projects/1/views/1) and look for an interesting problem you want to solve.

If you're new to this project, we recommend issues tagged with [<kbd>good first issue</kbd>](https://github.com/orgs/candy-lang/projects/1/views/4).
If there are no such issues (or too few), please write us [a message on Discord][Discord].

If an issue's title or description are not clear, please don't hesitate to comment on that issue and ask for clarification.

Once you have selected an issue to work on, assign yourself to that issue so we don't end up with two people doing the same thing. :)
If you don't have permissions to do this, just comment on that issue and we'll assign you.

If the issue is larger than expected and some parts of it can be fixed more easily in a self-contained manner, please open new issue(s) and link to them in the original issue.
You can then solve these smaller issue(s) on new branches.

## Working on Stuff

1. Create a branch for working on your assigned issue and switch to it locally:
   - If you have write access to the repository:
     On the right side of the issue page, under _Development_, click _Create a branch_, then click _Create branch_.
   - If you don't have write access to the repository:
     1. Fork the repository.
     2. Create a branch called `<issue ID>-issue-title-in-kebab-case`.
        E.g., for issue [#661](https://github.com/candy-lang/candy/issues/661) “Show more hints in IRs (values of constant etc.)”, the branch name should be `661-show-more-hints-in-irs-values-of-constant-etc`.
2. Implement your changes.
   Please add tests and make sure that there are no linter warnings.
3. Commit your changes.
   - You can also create smaller commits while you're working on your changes.
   - Commit messages should be in imperative mood and start with a capital letter, e.g., “Add contributing guide”
4. Push your changes to GitHub.
5. When you're done, file a pull request (PR).
   - Please give it a meaningful title and enter the corresponding issue number in the description.
   - On the right side, please select the corresponding type label(s) (called <kbd>T: …</kbd>, e.g., <kbd>T: Feature</kbd>).
   - Optionally, on the right side select reviewer(s).
   - The remaining fields will be filled out automatically.

We will take a look at your code, and, once all checks pass, your code can get merged 🏖️

## Pull Request Conversations

Regarding conversations (comments from code review) on pull requests:

- If you write a reply, _don't_ mark the conversation as resolved.
  Otherwise, the other person won't see your reply.
- If you are sure that you fully addressed a comment, _do_ mark the conversation as resolved.
  _Don't_ reply “Done” or something similar.

[Discord]: https://discord.gg/5Vr4eAJ7gU
