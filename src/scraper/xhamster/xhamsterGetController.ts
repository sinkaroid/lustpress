import { load } from "cheerio";
import LustPress from "../../LustPress";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

export async function scrapeContent(url: string) {
  try {
    const buffer = await lust.fetchBody(url);
    const $ = load(buffer.toString("utf8"));

    const raw = $("#initials-script").html();
    const initials = raw
      ? JSON.parse(
          raw.replace(/^window\.initials\s*=\s*/, "").replace(/;$/, ""),
        )
      : null;

    class Xhamster {
      link: string;
      id: string;
      title: string;
      image: string;
      duration: string;
      views: string;
      rating: string;
      upVote: string;
      downVote: string;
      publish: string;
      tags: string[];
      models: string[];
      video: string;

      constructor() {
        this.link = $("link[rel='canonical']").attr("href") || "None";
        this.id = this.link.split("/")[4] || "None";
        this.title = $("meta[property='og:title']").attr("content") || "None";
        this.image = $("meta[property='og:image']").attr("content") || "None";

        // defaults
        this.duration = "None";
        this.views = "None";

        const scripts = $("script")
          .map((i, el) => $(el).html())
          .get()
          .filter(Boolean);

        const videoScript = scripts.find(
          (s) => s.includes('"videoModel"') && s.includes('"duration"'),
        );

        if (videoScript) {
          const durMatch = videoScript.match(/"duration"\s*:\s*(\d+)/);
          if (durMatch) this.duration = durMatch[1];

          const viewMatch = videoScript.match(/"views"\s*:\s*(\d+)/);
          if (viewMatch) this.views = viewMatch[1];
        }

        this.rating =
          initials?.ratingComponent?.ratingModel?.value?.toString() || "None";
        this.upVote =
          initials?.ratingComponent?.ratingModel?.likes?.toString() || "None";
        this.downVote =
          initials?.ratingComponent?.ratingModel?.dislikes?.toString() ||
          "None";

        this.publish =
          $("div.entity-info-container__date").attr("data-tooltip") || "None";

        this.tags =
          initials?.videoTagsComponent?.tags
            ?.filter((t: any) => t.isTag)
            .map((t: any) => t.name) || [];

        this.models =
          initials?.videoTagsComponent?.tags
            ?.filter((t: any) => t.isPornstar)
            .map((t: any) => t.name) || [];

        const embedId = this.link.split("-").pop()?.replace("/", "");
        this.video = embedId ? `https://xhamster.com/embed/${embedId}` : "None";
      }
    }

    const xh = new Xhamster();

    return {
      success: true,
      data: {
        title: lust.removeHtmlTagWithoutSpace(xh.title),
        id: xh.id,
        image: xh.image,
        duration: xh.duration,
        views: xh.views,
        rating: xh.rating,
        uploaded: xh.publish,
        upvoted: xh.upVote,
        downvoted: xh.downVote,
        models: xh.models,
        tags: xh.tags,
      },
      source: xh.link,
      assets: [xh.video, xh.image],
    } satisfies IVideoData;
  } catch (err) {
    const e = err as Error;
    throw new Error(e.message);
  }
}
