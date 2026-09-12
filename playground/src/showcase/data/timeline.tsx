import {
  Timeline,
  TimelineConnector,
  TimelineContent,
  TimelineDate,
  TimelineDescription,
  TimelineIndicator,
  TimelineItem,
  TimelineSeparator,
  TimelineTitle,
} from "@/components/ui/timeline"
import { Demo, Section } from "../shared"
import { CheckIcon, CircleIcon } from "lucide-react"

export function TimelineSection() {
  return (
    <Section
      id="timeline"
      title="Timeline"
      group="examples"
      description="Displays a chronological sequence of events and activities."
      fileKey="data"
    >
      <Demo
        title="Timeline · Default"
        description="A timeline with date, title, and description."
      >
        <div className="w-full max-w-sm">
          <Timeline>
            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>January 2024</TimelineDate>
                <TimelineTitle>Project kickoff</TimelineTitle>
                <TimelineDescription>
                  Initial project planning and team alignment meeting.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>February 2024</TimelineDate>
                <TimelineTitle>Design phase</TimelineTitle>
                <TimelineDescription>
                  UI/UX designs completed and approved by stakeholders.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>March 2024</TimelineDate>
                <TimelineTitle>Development started</TimelineTitle>
                <TimelineDescription>
                  Frontend and backend implementation underway.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>April 2024</TimelineDate>
                <TimelineTitle>Beta release</TimelineTitle>
                <TimelineDescription>
                  First beta version deployed for internal testing.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>
          </Timeline>
        </div>
      </Demo>

      <Demo
        title="Timeline · Completed"
        description="A timeline showing completed milestones with check icons."
      >
        <div className="w-full max-w-sm">
          <Timeline>
            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator>
                  <CheckIcon className="size-3 text-primary-foreground" />
                </TimelineIndicator>
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>Completed</TimelineDate>
                <TimelineTitle>Project kickoff</TimelineTitle>
                <TimelineDescription>
                  Initial planning and team alignment.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator>
                  <CheckIcon className="size-3 text-primary-foreground" />
                </TimelineIndicator>
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>Completed</TimelineDate>
                <TimelineTitle>Design phase</TimelineTitle>
                <TimelineDescription>
                  UI/UX designs approved.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator>
                  <CircleIcon className="size-2 text-muted-foreground" />
                </TimelineIndicator>
              </TimelineSeparator>
              <TimelineContent>
                <TimelineDate>In Progress</TimelineDate>
                <TimelineTitle>Development</TimelineTitle>
                <TimelineDescription>
                  Currently building the application.
                </TimelineDescription>
              </TimelineContent>
            </TimelineItem>
          </Timeline>
        </div>
      </Demo>

      <Demo
        title="Timeline · Simple"
        description="A minimal timeline without dates."
      >
        <div className="w-full max-w-sm">
          <Timeline>
            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineTitle>First step</TimelineTitle>
                <TimelineDescription>Description for the first step.</TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
                <TimelineConnector />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineTitle>Second step</TimelineTitle>
                <TimelineDescription>Description for the second step.</TimelineDescription>
              </TimelineContent>
            </TimelineItem>

            <TimelineItem>
              <TimelineSeparator>
                <TimelineIndicator />
              </TimelineSeparator>
              <TimelineContent>
                <TimelineTitle>Third step</TimelineTitle>
                <TimelineDescription>Description for the third step.</TimelineDescription>
              </TimelineContent>
            </TimelineItem>
          </Timeline>
        </div>
      </Demo>
    </Section>
  )
}
