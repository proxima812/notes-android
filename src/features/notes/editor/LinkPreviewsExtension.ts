/**
 * Fetched titles and icons, brought into the editor.
 *
 * Three jobs, all of which have to happen outside a React render:
 *
 *   * ask the core about every address in the document, once;
 *   * draw the icon each link wears;
 *   * rename the links that are still showing a raw URL.
 *
 * The icon is a decoration rather than something the link mark renders, and
 * that is the whole reason this file is shaped the way it is. An answer from
 * the network changes nothing about the document, so ProseMirror will not
 * redraw a mark for it — the first icon a link was given would be the one it
 * kept for the rest of the session. A decoration set is recomputed and diffed
 * on every state change, which is exactly the behaviour this needs.
 */

import { Extension } from "@tiptap/react";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

import { askAbout, knownPreview, onPreviewsChanged } from "./linkPreviewStore";
import { linkRangesIn, showsItsOwnAddress } from "./linkRanges";
import { siteOf } from "./linkSites";

const key = new PluginKey<number>("linkPreviews");

export const LinkPreviews = Extension.create({
  name: "linkPreviews",

  addProseMirrorPlugins() {
    return [
      new Plugin<number>({
        key,

        state: {
          init: () => 0,
          // A counter, bumped whenever an answer lands. Its only purpose is to
          // make the plugin state differ, so the view updates and the
          // decorations above are rebuilt against what is now known.
          apply: (transaction, version) =>
            transaction.getMeta(key) === true ? version + 1 : version,
        },

        props: {
          decorations(state) {
            const decorations = linkRangesIn(state.doc).map((range) => {
              const preview = knownPreview(range.href);
              const icon = preview?.icon ?? null;
              // The site's own icon once it has been read; until then the mark
              // for a service the app draws itself, and the globe for the rest.
              const attributes =
                icon === null
                  ? { "data-link-site": siteOf(range.href) ?? "link" }
                  : { "data-link-icon": "", style: `--link-icon: url("${icon}")` };
              return Decoration.inline(range.from, range.to, attributes);
            });
            return DecorationSet.create(state.doc, decorations);
          },
        },

        // Runs on every change, which is where a newly pasted or newly typed
        // address is noticed, and where one that has an answer gets its name.
        appendTransaction(_transactions, _oldState, newState) {
          const ranges = linkRangesIn(newState.doc);
          for (const range of ranges) {
            askAbout(range.href);
          }

          const rename = ranges.find((range) => {
            const preview = knownPreview(range.href);
            return (
              preview !== undefined &&
              preview !== null &&
              preview.title !== null &&
              preview.title !== range.text &&
              showsItsOwnAddress(range.text, range.href)
            );
          });
          if (rename === undefined) {
            return null;
          }

          const title = knownPreview(rename.href)?.title;
          if (title === undefined || title === null) {
            return null;
          }

          // The marks come from the text being replaced, so the new words keep
          // the link — and whatever else the run was wearing.
          const marks = newState.doc.resolve(rename.from + 1).marks();
          return newState.tr.replaceWith(
            rename.from,
            rename.to,
            newState.schema.text(title, marks),
          );
        },

        view(view) {
          // The links a note already contains are asked about the moment it
          // opens. Without this only editing a note would ever give it icons,
          // and a note nobody is editing is the usual case for reading one.
          for (const range of linkRangesIn(view.state.doc)) {
            askAbout(range.href);
          }

          const stop = onPreviewsChanged(() => {
            if (view.isDestroyed) {
              return;
            }
            view.dispatch(view.state.tr.setMeta(key, true).setMeta("addToHistory", false));
          });
          return { destroy: stop };
        },
      }),
    ];
  },
});
